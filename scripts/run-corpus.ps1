# Run osp-analyzer on the 8 Rust/Go corpus repos and print a results table.
$ErrorActionPreference = 'Continue'
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$root = 'P:\Work\repos'
$analyzer = 'P:\Work\SoftwarePhysics\target\release\osp-analyze.exe'
$repos = @(
    @('serde',      'Rust'),
    @('ripgrep',    'Rust'),
    @('tracing',    'Rust'),
    @('tokio',      'Rust'),
    @('cobra',      'Go'),
    @('viper',      'Go'),
    @('gin',        'Go'),
    @('prometheus', 'Go')
)
Write-Output ("{0,-12} {1,-5} {2,-6} {3,-6} {4,-6} {5,-6} {6,-6} {7,-6}" -f 'repo','lang','nodes','edges','kappa','A','I','D')
foreach ($r in $repos) {
    $name = $r[0]; $lang = $r[1]
    $path = Join-Path $root $name
    if (-not (Test-Path $path)) { Write-Output "$name MISSING"; continue }
    $out = & $analyzer $path 2>$null | Select-String -Pattern "^\s*$name\s" | Select-Object -First 1
    if ($out) {
        Write-Output "$out".Trim()
    } else {
        Write-Output "$name : (no output)"
    }
}
