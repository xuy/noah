# Launch WITHOUT API key so the setup screen appears (for testing onboarding flows)
$appDir = "$env:APPDATA\com.itman.app"
Remove-Item "$appDir\api_key.txt" -ErrorAction SilentlyContinue
Remove-Item "$appDir\proxy.json" -ErrorAction SilentlyContinue

$env:ANTHROPIC_API_KEY = ""

Set-Location $PSScriptRoot
pnpm dev
