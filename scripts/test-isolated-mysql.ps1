param([switch]$KeepWork, [string]$TestFilter = '')

$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$auditRoot = Join-Path $projectRoot ('tmp\system-failure-audit-' + [Guid]::NewGuid().ToString('N'))
$mysqlBase = Join-Path $projectRoot 'src-tauri\resources\runtime\mysql'
$mysqlExe = Join-Path $mysqlBase 'bin\mysql.exe'
$mysqlAdminExe = Join-Path $mysqlBase 'bin\mysqladmin.exe'
$mysqldExe = Join-Path $mysqlBase 'bin\mysqld.exe'
$dataDirectory = Join-Path $auditRoot 'mysql-data'
$clientFile = Join-Path $auditRoot 'test-client.cnf'
$database = 'clinic_rust_test_audit_' + [Guid]::NewGuid().ToString('N')
$testPassword = 'IsolatedAuditOnly-20261112!'
$serverProcess = $null
$testExit = 1
$environmentNames = @('RUST_MYSQL_DATABASE', 'RUST_MYSQL_HOST', 'RUST_MYSQL_PORT', 'RUST_MYSQL_USER', 'RUST_MYSQL_PASSWORD', 'CLINIC_TEST_MYSQL_BASE')
$previousEnvironment = @{}
foreach ($name in $environmentNames) { $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }

function Get-TestPort {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    try { return $listener.LocalEndpoint.Port } finally { $listener.Stop() }
}

try {
    if (-not (Test-Path -LiteralPath $mysqldExe)) { throw 'Prepare the bundled Windows MySQL runtime before running these tests.' }
    New-Item -ItemType Directory -Path $dataDirectory -Force | Out-Null
    $mysqlPort = Get-TestPort
    Write-Host "Isolated test database: $database on port $mysqlPort"
    $initializer = Start-Process -FilePath $mysqldExe -ArgumentList @(
        '--no-defaults', '--initialize-insecure', "--basedir=`"$mysqlBase`"", "--datadir=`"$dataDirectory`"",
        "--log-error=`"$(Join-Path $auditRoot 'initialize.log')`""
    ) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $auditRoot 'initialize.stdout.log') -RedirectStandardError (Join-Path $auditRoot 'initialize.stderr.log')
    if (-not $initializer.WaitForExit(180000)) {
        $initializer.Kill($true)
        $initializer.WaitForExit(5000) | Out-Null
        throw 'The isolated MySQL initializer timed out.'
    }
    if ($initializer.ExitCode -ne 0) { throw "MySQL initialization failed; inspect $auditRoot" }

    # With skip-name-resolve, root@localhost cannot bootstrap through 127.0.0.1.
    # Provision only this temporary server through its startup SQL file instead.
    $bootstrapFile = Join-Path $auditRoot 'test-bootstrap.sql'
    $setupSql = @(
        "ALTER USER 'root'@'localhost' IDENTIFIED BY '$testPassword';"
        "CREATE USER 'root'@'127.0.0.1' IDENTIFIED BY '$testPassword';"
        "GRANT ALL PRIVILEGES ON *.* TO 'root'@'127.0.0.1' WITH GRANT OPTION;"
        "CREATE DATABASE ``$database`` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"
    ) -join "`n"
    [IO.File]::WriteAllText($bootstrapFile, $setupSql, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($clientFile, "[client]`nuser=root`npassword=$testPassword`nhost=127.0.0.1`nport=$mysqlPort`nprotocol=tcp`n", [Text.UTF8Encoding]::new($false))
    $serverProcess = Start-Process -FilePath $mysqldExe -ArgumentList @(
        '--no-defaults', "--basedir=`"$mysqlBase`"", "--datadir=`"$dataDirectory`"", "--port=$mysqlPort",
        '--bind-address=127.0.0.1', '--mysqlx=OFF', '--skip-name-resolve', '--innodb-buffer-pool-size=64M',
        "--init-file=`"$bootstrapFile`"",
        "--pid-file=`"$(Join-Path $auditRoot 'mysql.pid')`"", "--log-error=`"$(Join-Path $auditRoot 'mysql.log')`""
    ) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $auditRoot 'mysql.stdout.log') -RedirectStandardError (Join-Path $auditRoot 'mysql.stderr.log')
    $connection = @("--defaults-file=$clientFile", '--no-login-paths', '--connect-timeout=2')
    $deadline = [DateTime]::UtcNow.AddSeconds(120)
    $connected = $false
    while ([DateTime]::UtcNow -lt $deadline -and -not $serverProcess.HasExited) {
        & $mysqlExe @connection --batch --skip-column-names -e 'SELECT 1' 2>(Join-Path $auditRoot 'connection.stderr.log') | Out-Null
        if ($LASTEXITCODE -eq 0) { $connected = $true; break }
        Start-Sleep -Milliseconds 200
        $serverProcess.Refresh()
    }
    if (-not $connected) { throw "The isolated MySQL server did not become ready; inspect $auditRoot" }
    Remove-Item -LiteralPath $bootstrapFile
    [Environment]::SetEnvironmentVariable('RUST_MYSQL_DATABASE', $database, 'Process')
    [Environment]::SetEnvironmentVariable('RUST_MYSQL_HOST', '127.0.0.1', 'Process')
    [Environment]::SetEnvironmentVariable('RUST_MYSQL_PORT', [string]$mysqlPort, 'Process')
    [Environment]::SetEnvironmentVariable('RUST_MYSQL_USER', 'root', 'Process')
    [Environment]::SetEnvironmentVariable('RUST_MYSQL_PASSWORD', $testPassword, 'Process')
    [Environment]::SetEnvironmentVariable('CLINIC_TEST_MYSQL_BASE', $mysqlBase, 'Process')

    Push-Location (Join-Path $projectRoot 'src-tauri')
    try {
        $testArguments = @('test', '--offline', '--lib')
        if ($TestFilter) { $testArguments += $TestFilter }
        $testArguments += @('--', '--ignored', '--skip', 'writes_visual_review_fixtures', '--skip', 'child_', '--test-threads=1')
        & cargo @testArguments
        $testExit = $LASTEXITCODE
    } finally { Pop-Location }
} finally {
    foreach ($name in $environmentNames) { [Environment]::SetEnvironmentVariable($name, $previousEnvironment[$name], 'Process') }
    if ($serverProcess) {
        $serverProcess.Refresh()
        if (-not $serverProcess.HasExited -and (Test-Path -LiteralPath $clientFile)) {
            $stopper = Start-Process -FilePath $mysqlAdminExe -ArgumentList @("--defaults-file=`"$clientFile`"", '--no-login-paths', '--connect-timeout=3', 'shutdown') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $auditRoot 'shutdown.stdout.log') -RedirectStandardError (Join-Path $auditRoot 'shutdown.stderr.log')
            if (-not $stopper.WaitForExit(15000)) { $stopper.Kill($true); $stopper.WaitForExit(5000) | Out-Null }
        }
        if (-not $serverProcess.WaitForExit(10000)) { $serverProcess.Kill($true); $serverProcess.WaitForExit(5000) | Out-Null }
    }
    if ($testExit -eq 0 -and -not $KeepWork) {
        # Resolve and validate the exact owned directory before any recursive delete.
        $resolvedAudit = (Resolve-Path -LiteralPath $auditRoot).Path
        $allowedParent = [IO.Path]::GetFullPath((Join-Path $projectRoot 'tmp')) + [IO.Path]::DirectorySeparatorChar
        if (-not $resolvedAudit.StartsWith($allowedParent, [StringComparison]::OrdinalIgnoreCase)) { throw 'Refusing to remove a test directory outside this project.' }
        Remove-Item -LiteralPath $resolvedAudit -Recurse -Force
    } else { Write-Host "Kept isolated test files: $auditRoot" }
}
if ($testExit -ne 0) { throw "Isolated MySQL tests failed (exit $testExit)." }
