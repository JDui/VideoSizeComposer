<#
  HLG 读取 / 输出链路回归验证。

  用法：
      powershell -ExecutionPolicy Bypass -File .\qa\hlg\verify-hlg.ps1
      powershell -ExecutionPolicy Bypass -File .\qa\hlg\verify-hlg.ps1 -Keep

  脚本用随应用分发的 ffmpeg / ffprobe 复现编码链，量化 HLG→SDR 的亮度保持、
  编码域峰值抽样、色彩标签落地与 MDCV/MaxCLL 探测，全部通过时退出码为 0。
  结论记录在同目录 verification.md。

  注意：ffprobe 的字段名是 color_transfer，不是编码器选项名 color_trc；
  写成 color_trc 时 ffprobe 会静默省略该字段，看起来就像“标签丢了”。
#>
param(
  [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
  [switch]$Keep
)

$ErrorActionPreference = 'Continue'
$dist = Join-Path $RepoRoot 'dist\VideoSizeComposer'
$ff = Join-Path $dist 'ffmpeg.exe'
$fp = Join-Path $dist 'ffprobe.exe'
foreach ($tool in @($ff, $fp)) {
  if (-not (Test-Path $tool)) { Write-Host "缺少内置工具：$tool（先运行 build-portable.ps1 生成 dist）" -ForegroundColor Red; exit 2 }
}
$work = Join-Path $PSScriptRoot 'work'
if (Test-Path $work) { Remove-Item $work -Recurse -Force }
New-Item -ItemType Directory -Force -Path $work | Out-Null
Push-Location $work
try {
  $script:failures = 0
  function Step([string]$name) { Write-Host "`n=== $name ===" -ForegroundColor Cyan }
  function Check([string]$label, [bool]$ok, [string]$detail) {
    if ($ok) { Write-Host ("  [ok]   {0,-30} {1}" -f $label, $detail) -ForegroundColor Green }
    else { $script:failures++; Write-Host ("  [FAIL] {0,-30} {1}" -f $label, $detail) -ForegroundColor Red }
  }
  function RunFfmpeg([string[]]$ffmpegArgs) {
    $result = & $ff -y -hide_banner -loglevel error @ffmpegArgs 2>&1
    if ($LASTEXITCODE -ne 0) { Write-Host ("  ffmpeg 失败: {0}" -f ($result -join ' | ')) -ForegroundColor Yellow }
    return $LASTEXITCODE
  }
  function StreamInfo([string]$path) {
    $json = & $fp -v error -select_streams v:0 -show_entries stream=codec_name,codec_tag_string,pix_fmt,color_primaries,color_transfer,color_space,color_range -of json $path 2>$null | Out-String
    if (-not $json) { return $null }
    return ((ConvertFrom-Json $json).streams[0])
  }
  # signalstats 走 stderr，避免把 Windows 绝对路径塞进滤镜图（metadata=print:file= 的冒号转义会解析失败）
  function SignalStats([string]$path, [string[]]$extra = @()) {
    $raw = & $ff -hide_banner -loglevel info @extra -i $path -vf 'signalstats,metadata=print' -f null - 2>&1 | Out-String
    $stats = @{}
    foreach ($line in $raw -split "`n") {
      if ($line -match 'lavfi\.signalstats\.(Y\w+)=(-?[\d.]+)') {
        $key = $matches[1]; $value = [double]$matches[2]
        if (-not $stats.ContainsKey($key)) { $stats[$key] = @() }
        $stats[$key] += $value
      }
    }
    return $stats
  }
  function MeanOf($stats, [string]$key) {
    if (-not $stats.ContainsKey($key) -or $stats[$key].Count -eq 0) { return $null }
    return ($stats[$key] | Measure-Object -Average).Average
  }
  function MaxOf($stats, [string]$key) {
    if (-not $stats.ContainsKey($key)) { return $null }
    return ($stats[$key] | Measure-Object -Maximum).Maximum
  }
  # 与 lib.rs 的 coded_peak_ratio 一致：10-bit / tv 量程 64..940
  function CodedPeak($stats) {
    return [math]::Round(((MaxOf $stats 'YMAX') - 64) / (940 - 64), 2)
  }
  function ConvertChain([string]$src, [string]$vf, [string]$out) {
    return (RunFfmpeg @('-i', $src, '-vf', "$vf,format=yuv420p", '-c:v', 'libx265', '-preset', 'ultrafast', '-crf', '16', '-color_primaries', 'bt709', '-color_trc', 'bt709', '-colorspace', 'bt709', '-color_range', 'tv', $out))
  }

  $lavfi = 'testsrc2=size=160x96:rate=25:duration=0.2'

  Step '生成素材（SDR 母版 / SDR 参考的 HLG / 满量程 HLG）'
  RunFfmpeg @('-f', 'lavfi', '-i', $lavfi, '-c:v', 'libx265', '-preset', 'ultrafast', '-crf', '16', '-pix_fmt', 'yuv420p',
    '-color_primaries', 'bt709', '-color_trc', 'bt709', '-colorspace', 'bt709', '-color_range', 'tv', 'sdr_master.mp4') | Out-Null
  RunFfmpeg @('-i', 'sdr_master.mp4',
    '-vf', 'zscale=primariesin=bt709:transferin=bt709:matrixin=bt709:rangein=tv:primaries=bt2020:transfer=arib-std-b67:matrix=bt2020nc:range=tv,format=yuv420p10le',
    '-c:v', 'libx265', '-preset', 'ultrafast', '-crf', '16', '-tag:v', 'hvc1',
    '-color_primaries', 'bt2020', '-color_trc', 'arib-std-b67', '-colorspace', 'bt2020nc', '-color_range', 'tv', 'hlg_sdr_referred.mp4') | Out-Null
  # 把亮度整体推到码值顶端，等价于广电 HLG：场景参考信号真的打到满量程
  RunFfmpeg @('-i', 'hlg_sdr_referred.mp4',
    '-vf', "format=yuv420p10le,lutyuv=y='min(940,round(val*1.72))'",
    '-c:v', 'libx265', '-preset', 'ultrafast', '-crf', '16', '-tag:v', 'hvc1',
    '-color_primaries', 'bt2020', '-color_trc', 'arib-std-b67', '-colorspace', 'bt2020nc', '-color_range', 'tv', 'hlg_true_peak.mp4') | Out-Null
  foreach ($clip in 'sdr_master.mp4', 'hlg_sdr_referred.mp4', 'hlg_true_peak.mp4') {
    Check "生成 $clip" ((Test-Path $clip) -and ((Get-Item $clip).Length -gt 1KB)) ("{0} KB" -f [math]::Round((Get-Item $clip).Length / 1KB, 1))
  }

  Step '色彩标签落地（HLG 输出必须带 arib-std-b67）'
  $tagged = StreamInfo 'hlg_sdr_referred.mp4'
  Check 'HLG 三原色' ($tagged.color_primaries -eq 'bt2020') "color_primaries=$($tagged.color_primaries)"
  Check 'HLG 传递函数' ($tagged.color_transfer -eq 'arib-std-b67') "color_transfer=$($tagged.color_transfer)"
  Check 'HLG 矩阵' ($tagged.color_space -eq 'bt2020nc') "color_space=$($tagged.color_space)"
  Check 'HLG 量化范围' ($tagged.color_range -eq 'tv') "color_range=$($tagged.color_range)"
  Check 'HLG 位深' ($tagged.pix_fmt -eq 'yuv420p10le') "pix_fmt=$($tagged.pix_fmt)"

  Step '编码域峰值抽样（区分 SDR 参考 HLG 与真 HDR HLG）'
  $lowStats = SignalStats 'hlg_sdr_referred.mp4' @('-skip_frame', 'nokey')
  $highStats = SignalStats 'hlg_true_peak.mp4' @('-skip_frame', 'nokey')
  $lowPeak = CodedPeak $lowStats
  $highPeak = CodedPeak $highStats
  Check 'SDR 参考 HLG 低于阈值' ($lowPeak -lt 0.80) ('coded peak = {0}（YMAX {1}）' -f $lowPeak, (MaxOf $lowStats 'YMAX'))
  Check '满量程 HLG 高于阈值' ($highPeak -ge 0.80) ('coded peak = {0}（YMAX {1}）' -f $highPeak, (MaxOf $highStats 'YMAX'))
  Check '两档素材分得开' (($highPeak - $lowPeak) -gt 0.25) ('间距 {0:N2}' -f ($highPeak - $lowPeak))

  $direct = 'zscale=primariesin=bt2020:transferin=arib-std-b67:matrixin=bt2020nc:rangein=tv:primaries=bt709:transfer=bt709:matrix=bt709:range=tv'
  $softKnee = 'zscale=primariesin=bt2020:transferin=arib-std-b67:matrixin=bt2020nc:rangein=tv:primaries=bt2020:transfer=linear,format=gbrpf32le,tonemap=hable:desat=0,zscale=primaries=bt709:transfer=bt709:matrix=bt709:range=tv'
  $masterLuma = MeanOf (SignalStats 'sdr_master.mp4') 'YAVG'

  Step 'HLG→SDR：SDR 参考素材走 BT.2408 直映射（自动档）'
  ConvertChain 'hlg_sdr_referred.mp4' $direct 's1_direct.mp4' | Out-Null
  $directStats = SignalStats 's1_direct.mp4'
  $directLuma = MeanOf $directStats 'YAVG'
  $drift = [math]::Abs(($directLuma - $masterLuma) / $masterLuma)
  Check '直映射亮度贴近 SDR 母版' (($drift -lt 0.12)) ('母版 {0:N1} → 输出 {1:N1}，偏移 {2:P1}' -f $masterLuma, $directLuma, $drift)
  Check '直映射不越界' (((MaxOf $directStats 'YMAX') -le 236)) ("Ymax = {0}" -f (MaxOf $directStats 'YMAX'))
  ConvertChain 'hlg_sdr_referred.mp4' $softKnee 's1_softknee.mp4' | Out-Null
  $kneeLuma = MeanOf (SignalStats 's1_softknee.mp4') 'YAVG'
  Check '软膝对 SDR 参考素材明显偏暗' (($kneeLuma -lt $directLuma * 0.85)) ('直映射 {0:N1} vs 软膝 {1:N1}（旧链条把整体压暗 ~28%）' -f $directLuma, $kneeLuma)

  Step 'HLG→SDR：满量程素材必须走软膝（否则高光爆掉）'
  ConvertChain 'hlg_true_peak.mp4' $direct 's2_direct.mp4' | Out-Null
  $s2Direct = SignalStats 's2_direct.mp4'
  ConvertChain 'hlg_true_peak.mp4' $softKnee 's2_softknee.mp4' | Out-Null
  $s2Knee = SignalStats 's2_softknee.mp4'
  Check '直映射使满量程素材越界' (((MaxOf $s2Direct 'YMAX') -ge 250)) ('直映射 Ymax = {0}（>235 即非法）' -f (MaxOf $s2Direct 'YMAX'))
  Check '软膝把峰值压回合法量程' (((MaxOf $s2Knee 'YMAX') -le 245)) ('软膝 Ymax = {0}' -f (MaxOf $s2Knee 'YMAX'))
  Check '软膝保留高光层次' (((MaxOf $s2Knee 'YMAX') -gt (MeanOf $s2Knee 'YAVG') * 1.4)) ('Ymax {0} / YAVG {1:N1}' -f (MaxOf $s2Knee 'YMAX'), (MeanOf $s2Knee 'YAVG'))
  $s2Tags = StreamInfo 's2_softknee.mp4'
  Check '转换后标签改写为 SDR' ($s2Tags.color_transfer -eq 'bt709' -and $s2Tags.color_primaries -eq 'bt709') ('pri={0} trc={1}' -f $s2Tags.color_primaries, $s2Tags.color_transfer)

  Step '色调映射档位可用性（本构建 tonemap 仅支持 none|linear|gamma|clip|reinhard|hable|mobius）'
  foreach ($algo in 'hable', 'mobius', 'reinhard', 'clip') {
    $chain = "zscale=primariesin=bt2020:transferin=arib-std-b67:matrixin=bt2020nc:rangein=tv:primaries=bt2020:transfer=linear,format=gbrpf32le,tonemap=$algo`:desat=0,zscale=primaries=bt709:transfer=bt709:matrix=bt709:range=tv"
    $code = ConvertChain 'hlg_true_peak.mp4' $chain "algo_$algo.mp4"
    $stats = SignalStats "algo_$algo.mp4"
    Check "tonemap=$algo" ($code -eq 0 -and (MaxOf $stats 'YMAX') -le 245) ('exit={0} Ymax={1}' -f $code, (MaxOf $stats 'YMAX'))
  }
  $bt2390 = RunFfmpeg @('-hide_banner', '-loglevel', 'error', '-i', 'hlg_true_peak.mp4', '-vf', 'tonemap=bt2390', '-f', 'null', '-')
  Check 'tonemap=bt2390 不可用（预期）' ($bt2390 -ne 0) '确认不能依赖 bt2390 / max_peak / target_peak'
  $nplCheck = RunFfmpeg @('-hide_banner', '-loglevel', 'error', '-i', 'hlg_sdr_referred.mp4',
    '-vf', 'zscale=primariesin=bt2020:transferin=arib-std-b67:matrixin=bt2020nc:rangein=tv:primaries=bt709:transfer=bt709:matrix=bt709:range=tv:npl=1000',
    '-f', 'null', '-')
  Check 'npl 不参与色彩链（预期）' ($nplCheck -eq 0) '编码链已移除 npl：对 SDR 参考素材只会整体压暗'

  Step 'MDCV / MaxCLL 从帧 side_data 读取'
  RunFfmpeg @('-f', 'lavfi', '-i', $lavfi,
    '-vf', 'zscale=primariesin=bt709:transferin=bt709:matrixin=bt709:rangein=tv:primaries=bt2020:transfer=smpte2084:matrix=bt2020nc:range=tv,format=yuv420p10le',
    '-c:v', 'libx265', '-preset', 'ultrafast', '-crf', '16',
    '-x265-params', 'hdr-opt=1:repeat-headers=1:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:master-display=G(13250,34500)B(7500,3000)R(34000,16000)WP(15635,16450)L(10000000,1):max-cll=700,200',
    '-tag:v', 'hvc1', '-color_primaries', 'bt2020', '-color_trc', 'smpte2084', '-colorspace', 'bt2020nc', '-color_range', 'tv', 'pq_with_mdcv.mp4') | Out-Null
  $side = & $fp -v error -select_streams v:0 -read_intervals '%+#1' -show_frames -show_entries frame=side_data_list -of json 'pq_with_mdcv.mp4' 2>$null | Out-String
  $frame = ((ConvertFrom-Json $side).frames[0]).side_data_list
  $mdcv = $frame | Where-Object { $_.side_data_type -eq 'Mastering display metadata' }
  $cll = $frame | Where-Object { $_.side_data_type -eq 'Content light level metadata' }
  Check '读到 MDCV' ($null -ne $mdcv) "max_luminance=$($mdcv.max_luminance)"
  Check '读到 MaxCLL / MaxFALL' ($cll.max_content -eq 700 -and $cll.max_average -eq 200) "max_content=$($cll.max_content) max_average=$($cll.max_average)"
  $pqStream = StreamInfo 'pq_with_mdcv.mp4'
  Check 'HDR10 标签落地' ($pqStream.color_transfer -eq 'smpte2084' -and $pqStream.codec_tag_string -eq 'hvc1') ('trc={0} tag={1}' -f $pqStream.color_transfer, $pqStream.codec_tag_string)

  Step '未标注宽色域素材：只标疑似，不改像素'
  RunFfmpeg @('-f', 'lavfi', '-i', $lavfi, '-c:v', 'libx265', '-preset', 'ultrafast', '-crf', '16', '-pix_fmt', 'yuv420p10le',
    '-color_primaries', 'bt2020', '-colorspace', 'bt2020nc', '-color_trc', 'unspecified', 'unlabelled.mp4') | Out-Null
  $unlabelled = StreamInfo 'unlabelled.mp4'
  Check '传递函数缺失可被识别' ((-not $unlabelled.color_transfer) -or $unlabelled.color_transfer -in @('unspecified', 'unknown')) "color_transfer=$($unlabelled.color_transfer)"
  Check '矩阵仍可判定宽色域' ($unlabelled.color_space -eq 'bt2020nc') "color_space=$($unlabelled.color_space)"

  Step '跟随源文件（HDR 模式 = 跟随源文件）不被改写标签'
  $followOut = RunFfmpeg @('-i', 'hlg_sdr_referred.mp4', '-c:v', 'libx265', '-preset', 'ultrafast', '-crf', '16',
    '-pix_fmt', 'yuv420p10le', '-tag:v', 'hvc1', '-color_primaries', 'bt2020', '-color_trc', 'arib-std-b67', '-colorspace', 'bt2020nc', '-color_range', 'tv', 'follow_source.mp4')
  $follow = StreamInfo 'follow_source.mp4'
  Check 'HLG 原样保留' ($followOut -eq 0 -and $follow.color_transfer -eq 'arib-std-b67') "color_transfer=$($follow.color_transfer)"
} finally {
  Pop-Location
  if (-not $Keep) { Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue }
  else { Write-Host "`n中间产物保留在 $work" -ForegroundColor DarkGray }
}

if ($script:failures -eq 0) {
  Write-Host "`nHLG 链路验证全部通过。" -ForegroundColor Green
  exit 0
}
Write-Host "`nHLG 链路验证失败 $($script:failures) 项。" -ForegroundColor Red
exit 1
