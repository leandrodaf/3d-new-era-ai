# Installs 3D New Era AI on Windows, for your user only (no administrator).
#
#   irm https://raw.githubusercontent.com/leandrodaf/3d-new-era-ai/main/scripts/install-windows.ps1 | iex
#
# Downloads the latest release into %LOCALAPPDATA%\Programs\3D New Era AI,
# adds Start menu and desktop shortcuts, puts `newera` on your user PATH,
# registers an uninstaller in Settings > Apps and the MCP server in Claude
# Code when it is installed. Files fetched this way carry no "downloaded from
# the internet" mark, so SmartScreen doesn't stop the app. Running it again
# updates; nothing is downloaded when you already have the latest version.
# Temporary files are deleted at the end, success or failure.
#
# Uninstall: Settings > Apps > 3D New Era AI, or
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/leandrodaf/3d-new-era-ai/main/scripts/install-windows.ps1))) -Uninstall

param([switch]$Uninstall, [switch]$Force)

function Install-NewEra {
    param([switch]$Uninstall, [switch]$Force)
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'  # Invoke-WebRequest is much faster without it
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

    $repo = 'leandrodaf/3d-new-era-ai'
    $name = '3D New Era AI'
    $dir = Join-Path $env:LOCALAPPDATA "Programs\$name"
    $startMenu = Join-Path ([Environment]::GetFolderPath('Programs')) "$name.lnk"
    $desktop = Join-Path ([Environment]::GetFolderPath('Desktop')) "$name.lnk"
    $uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\NewEraAI'

    function Step($text) { Write-Host "`n==> $text" -ForegroundColor Cyan }
    function Info($text) { Write-Host "    $text" }

    function Remove-FromUserPath($folder) {
        $path = [Environment]::GetEnvironmentVariable('Path', 'User')
        if ($path) {
            $kept = ($path -split ';') | Where-Object { $_ -and ($_.TrimEnd('\') -ne $folder.TrimEnd('\')) }
            [Environment]::SetEnvironmentVariable('Path', ($kept -join ';'), 'User')
        }
    }

    if (Get-Process -Name 'newera-gui', 'newera' -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith($dir, [StringComparison]::OrdinalIgnoreCase) }) {
        throw "Feche o 3D New Era AI antes de continuar."
    }

    if ($Uninstall) {
        Step 'Desinstalando'
        Remove-Item -Recurse -Force $dir, $startMenu, $desktop -ErrorAction SilentlyContinue
        Remove-Item -Recurse -Force $uninstallKey -ErrorAction SilentlyContinue
        Remove-FromUserPath $dir
        if (Get-Command claude -ErrorAction SilentlyContinue) {
            claude mcp remove --scope user newera 2>$null | Out-Null
        }
        if (Get-Command codex -ErrorAction SilentlyContinue) {
            codex mcp remove newera 2>$null | Out-Null
        }
        Info "removido: $dir, atalhos, PATH e o MCP 'newera' do Claude Code e do Codex"
        return
    }

    Step 'Última versão'
    # github.com/<repo>/releases/latest redirects to the newest tag. Unlike the
    # REST API it has no per-IP limit, so offices behind one IP aren't blocked.
    $request = [Net.HttpWebRequest]::Create("https://github.com/$repo/releases/latest")
    $request.AllowAutoRedirect = $false
    $request.UserAgent = 'newera-installer'
    $response = $request.GetResponse()
    $location = $response.Headers['Location']
    $response.Close()
    if (-not $location -or $location -notmatch '/releases/tag/([^/?#]+)') { throw 'Nenhuma versão publicada ainda.' }
    $tag = [Uri]::UnescapeDataString($Matches[1])
    $assetName = 'newera-windows-x64.zip'
    Info $tag
    $versionFile = Join-Path $dir 'version.txt'
    if (-not $Force -and (Test-Path (Join-Path $dir 'newera-gui.exe')) -and
        (Test-Path $versionFile) -and ((Get-Content $versionFile -Raw).Trim() -eq $tag)) {
        Info 'já instalado nesta versão: nada a baixar (use -Force para reinstalar)'
    }
    else {
        $work = Join-Path ([IO.Path]::GetTempPath()) ("newera-install-" + [Guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $work | Out-Null
        try {
            Step "Baixando $assetName"
            $zip = Join-Path $work 'newera.zip'
            try {
                Invoke-WebRequest "https://github.com/$repo/releases/download/$tag/$assetName" -OutFile $zip -UseBasicParsing
            }
            catch { throw "A versão $tag não tem o pacote para Windows ($assetName)." }
            Expand-Archive $zip -DestinationPath $work
            $unpacked = Join-Path $work $name
            & (Join-Path $unpacked 'newera.exe') --version | Out-Null
            if ($LASTEXITCODE -ne 0) { throw 'O programa baixado não abriu neste computador.' }

            Step "Instalando em $dir"
            Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
            New-Item -ItemType Directory -Path (Split-Path $dir) -Force | Out-Null
            Move-Item $unpacked $dir
            Set-Content -Path $versionFile -Value $tag -Encoding ascii
            Info 'ok'
        }
        finally {
            Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
        }
    }

    Step 'Atalhos'
    $gui = Join-Path $dir 'newera-gui.exe'
    $shell = New-Object -ComObject WScript.Shell
    foreach ($link in @($startMenu, $desktop)) {
        $shortcut = $shell.CreateShortcut($link)
        $shortcut.TargetPath = $gui
        $shortcut.WorkingDirectory = [Environment]::GetFolderPath('MyDocuments')
        $shortcut.Description = 'Home design with a built-in MCP server'
        $shortcut.Save()
    }
    Info 'Menu Iniciar e Área de Trabalho'

    Step 'Comando newera no PATH do usuário'
    $path = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not (($path -split ';') | Where-Object { $_.TrimEnd('\') -eq $dir.TrimEnd('\') })) {
        [Environment]::SetEnvironmentVariable('Path', ((@($path, $dir) | Where-Object { $_ }) -join ';'), 'User')
        Info 'adicionado (abra um terminal novo)'
    }
    else { Info 'ok' }

    # Settings > Apps lists it, and its Uninstall button runs this same script.
    $uninstaller = Join-Path $dir 'uninstall.ps1'
    Set-Content -Path $uninstaller -Encoding utf8 -Value (
        "& ([scriptblock]::Create((Invoke-RestMethod 'https://raw.githubusercontent.com/$repo/main/scripts/install-windows.ps1'))) -Uninstall")
    New-Item -Path $uninstallKey -Force | Out-Null
    $version = $tag.TrimStart('v')
    @{
        DisplayName     = $name
        DisplayVersion  = $version
        Publisher       = '3D New Era AI'
        DisplayIcon     = $gui
        InstallLocation = $dir
        UninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$uninstaller`""
        URLInfoAbout    = "https://github.com/$repo"
    }.GetEnumerator() | ForEach-Object { New-ItemProperty -Path $uninstallKey -Name $_.Key -Value $_.Value -Force | Out-Null }
    New-ItemProperty -Path $uninstallKey -Name NoModify -Value 1 -PropertyType DWord -Force | Out-Null

    $mcpUrl = 'http://127.0.0.1:7878/mcp'
    if (Get-Command claude -ErrorAction SilentlyContinue) {
        Step 'MCP no Claude Code'
        claude mcp get newera 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { Info 'já registrado' }
        else {
            claude mcp add --scope user --transport http newera $mcpUrl 2>$null | Out-Null
            if ($LASTEXITCODE -eq 0) { Info "servidor 'newera' registrado" }
            else { Info "não consegui registrar; rode: claude mcp add --transport http newera $mcpUrl" }
        }
    }
    if (Get-Command codex -ErrorAction SilentlyContinue) {
        Step 'MCP no Codex'
        codex mcp get newera 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { Info 'já registrado' }
        else {
            codex mcp add newera --url $mcpUrl 2>$null | Out-Null
            if ($LASTEXITCODE -eq 0) { Info "servidor 'newera' registrado" }
            else { Info "não consegui registrar; rode: codex mcp add newera --url $mcpUrl" }
        }
    }

    Write-Host "`nPronto!" -ForegroundColor Green
    Write-Host "  Abrir:        Menu Iniciar ou Área de Trabalho > $name"
    Write-Host '  Terminal:     newera --demo'
    Write-Host '  MCP:          http://127.0.0.1:7878/mcp (com o editor aberto; outras IAs: veja o README)'
    Write-Host '  Atualizar:    rode o mesmo comando de novo'
    Write-Host '  Desinstalar:  Configurações > Aplicativos > 3D New Era AI'
}

Install-NewEra -Uninstall:$Uninstall -Force:$Force
