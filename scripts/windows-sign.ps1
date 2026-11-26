# Signs one file with Azure Artifact Signing; Tauri runs it on the app and both installers.
# The signing client signs in with a token GitHub issues for this run (OpenID Connect), so no
# Azure secret is stored. A fresh token is requested for each file, because the Windows build
# can outlast one.
param([Parameter(Mandatory = $true)][string]$File)
$ErrorActionPreference = 'Stop'

$request = "$($env:ACTIONS_ID_TOKEN_REQUEST_URL)&audience=api://AzureADTokenExchange"
$token = (Invoke-RestMethod -Uri $request -Headers @{ Authorization = "Bearer $($env:ACTIONS_ID_TOKEN_REQUEST_TOKEN)" }).value
$tokenFile = Join-Path $env:RUNNER_TEMP 'azure-federated-token'
Set-Content -Path $tokenFile -Value $token -NoNewline
$env:AZURE_FEDERATED_TOKEN_FILE = $tokenFile

& $env:ARRUMO_SIGNTOOL sign /v /fd SHA256 /tr 'http://timestamp.acs.microsoft.com' /td SHA256 `
  /d 'Arrumo AI Video Editor' /dlib $env:ARRUMO_SIGNING_DLIB /dmdf $env:ARRUMO_SIGNING_METADATA $File
exit $LASTEXITCODE
