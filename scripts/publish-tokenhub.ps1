<#
  Publish this project to GitHub as "TokenHub".
  Prereqs: Git + GitHub CLI (gh) installed, and `gh auth login` done once.
  Run:   powershell -ExecutionPolicy Bypass -File scripts\publish-tokenhub.ps1
  Private instead of public:   ... publish-tokenhub.ps1 -Private
#>
param([string]$RepoName = "TokenHub", [switch]$Private)
$ErrorActionPreference = "Stop"

# Always operate from the repo root (parent of this script's folder).
Set-Location (Split-Path $PSScriptRoot -Parent)

foreach ($t in @("git","gh")) {
  if (-not (Get-Command $t -ErrorAction SilentlyContinue)) {
    throw "$t not found. Install Git and the GitHub CLI (gh), then re-run."
  }
}
gh auth status *> $null
if ($LASTEXITCODE -ne 0) { throw "Not logged in. Run 'gh auth login' once, then re-run this script." }

# Fresh, valid repo (repair a partial .git if one was left behind).
git rev-parse --git-dir *> $null
if ($LASTEXITCODE -ne 0) {
  if (Test-Path .git) { Remove-Item -Recurse -Force .git }
  git init -b main | Out-Null
}

git add -A
git commit -m "TokenHub" *> $null   # no-op if nothing changed
Write-Host "Committed $(git rev-list --count HEAD 2>$null) revision(s)."

$vis = if ($Private) { "--private" } else { "--public" }
gh repo create $RepoName --source=. --remote=origin $vis --push

$url = gh repo view --json url -q .url
Write-Host "`nDone. Published to $url"
