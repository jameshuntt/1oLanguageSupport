#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"; cd "$ROOT"
LOG="$ROOT/PUBLISH_1O_RESULT.txt"
exec > >(tee "$LOG") 2>&1
fail(){ printf '\n1o RELEASE STOPPED: %s\nReport: %s\n' "$1" "$LOG"; exit 1; }
REPO_SLUG='jameshuntt/1oLanguageSupport'
REPO_URL='https://github.com/jameshuntt/1oLanguageSupport.git'

printf '\n=== 1o COMPLETE RELEASE ===\n'
printf 'GitHub source + core-1o + language-server-1o + existing 1o.1o Marketplace extension\n\n'

for t in git node npm python3 cargo; do command -v "$t" >/dev/null 2>&1 || fail "$t is missing"; done
if ! command -v gh >/dev/null 2>&1; then
  command -v brew >/dev/null 2>&1 || fail 'GitHub CLI is missing and Homebrew is unavailable.'
  brew install gh || fail 'Could not install GitHub CLI.'
fi
if ! gh auth status --hostname github.com >/dev/null 2>&1; then
  printf '[setup] GitHub sign-in will open once.\n'
  gh auth login --hostname github.com --git-protocol https --web || fail 'GitHub sign-in failed.'
fi
gh auth setup-git >/dev/null 2>&1 || fail 'Could not configure GitHub authentication for git.'

VERSION="$(node -p 'require("./package.json").version')"
CORE_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' crates/core-1o/Cargo.toml | head -n1)"
SERVER_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' crates/language-server-1o/Cargo.toml | head -n1)"
TAG="v$VERSION"
DEFAULT="$(gh repo view "$REPO_SLUG" --json defaultBranchRef --jq '.defaultBranchRef.name')"
[ -n "$DEFAULT" ] || fail 'Could not determine the repository default branch.'

printf '[1/8] Local source/build/LSP/package gate...\n'
python3 scripts/verify_source.py || fail 'Source audit failed.'
cargo test --workspace --lib || fail 'Rust tests failed.'
cargo build --release -p language-server-1o || fail 'Native server build failed.'
ONEO_VERIFY_SERVER_PATH="$ROOT/target/release/language-server-1o" node scripts/verify_1o_installed.mjs "$ROOT" || fail 'Live LSP/schema/AST test failed.'
npm ci --no-audit --no-fund || fail 'Extension dependencies failed.'
npm run compile || fail 'Extension compile failed.'
LOCAL_ARCH="$(node -p 'process.arch')"
if [ "$(uname -s)" = Darwin ]; then
  LOCAL_TARGET="darwin-${LOCAL_ARCH}"
  LOCAL_EXE="$ROOT/target/release/language-server-1o"
  LOCAL_VSIX="$ROOT/.release-package-stage/local-preflight-${LOCAL_TARGET}.vsix"
  printf 'Checking actual native Marketplace packaging locally BEFORE starting GitHub builds...\n'
  node scripts/package_marketplace_target.mjs --target "$LOCAL_TARGET" --runtime "$LOCAL_TARGET" --binary "$LOCAL_EXE" --out "$LOCAL_VSIX" || fail 'Marketplace packaging preflight failed; no GitHub jobs started.'
  python3 scripts/verify_single_vsix.py "$LOCAL_VSIX" "$LOCAL_TARGET" || fail 'Local VSIX content verification failed.'
fi
python3 scripts/publish_crates.py --verify-only || fail 'crates.io preflight failed.'
if gh release view "$TAG" --repo "$REPO_SLUG" >/dev/null 2>&1; then fail "GitHub Release $TAG already exists."; fi
printf 'PASS local release gate\n'

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
BRANCH="1o-release-${VERSION}-${STAMP}"
WORK="$HOME/Downloads/1O_RELEASE_${VERSION}_${STAMP}"
rm -rf "$WORK"; mkdir -p "$WORK"

printf '[2/8] Stage the finished source into your existing GitHub repository...\n'
git clone "$REPO_URL" "$WORK/repo" >/dev/null 2>&1 || fail 'Could not clone 1oLanguageSupport.'
cd "$WORK/repo"
git checkout "$DEFAULT" >/dev/null 2>&1 || fail "Could not check out $DEFAULT."
git pull --ff-only origin "$DEFAULT" >/dev/null 2>&1 || fail 'Could not update the repository safely.'
BASE_SHA="$(git rev-parse HEAD)"
git checkout -b "$BRANCH" >/dev/null || fail 'Could not create release branch.'
python3 "$ROOT/scripts/stage_repository.py" --source "$ROOT" --repo "$WORK/repo" || fail 'Could not stage canonical source.'
git add -A
# --allow-empty: when the source is already on the default branch, the release commit is still made
git -c user.name='1o release' -c user.email='release@1o.local' commit --allow-empty -m "Release 1o $VERSION" >/dev/null || fail 'No release commit could be created.'
RELEASE_SHA="$(git rev-parse HEAD)"
git push -u origin "$BRANCH" >/dev/null || fail 'Could not send the release source to GitHub.'
printf 'PASS source sent to GitHub for cross-platform qualification\n'

printf '[3/8] Wait for macOS, Windows, Linux, ARM and Alpine builds...\n'
RUN_ID=''
for _ in $(seq 1 60); do
  RUN_ID="$(gh run list --repo "$REPO_SLUG" --branch "$BRANCH" --limit 30 --json databaseId,workflowName,headSha --jq ".[] | select(.workflowName==\"1o all-platform release build\" and .headSha==\"$RELEASE_SHA\") | .databaseId" 2>/dev/null | head -n1 || true)"
  [ -n "$RUN_ID" ] && break
  sleep 3
done
[ -n "$RUN_ID" ] || fail 'GitHub did not start the all-platform build.'
CI_LOG="$ROOT/CI_RUN_${RUN_ID}.txt"
if ! gh run watch "$RUN_ID" --repo "$REPO_SLUG" --exit-status --interval 15 > "$CI_LOG" 2>&1; then
  printf 'ERROR: Cross-platform build failed. Fetching actual failing steps...\n'
  gh run view "$RUN_ID" --repo "$REPO_SLUG" --log-failed > "$ROOT/CI_FAILED_STEPS.txt" 2>&1 || true
  printf 'GitHub run: https://github.com/%s/actions/runs/%s\n' "$REPO_SLUG" "$RUN_ID"
  if [ -s "$ROOT/CI_FAILED_STEPS.txt" ]; then tail -n 100 "$ROOT/CI_FAILED_STEPS.txt"; else tail -n 100 "$CI_LOG"; fi
  fail 'At least one platform failed. No publication. Inspect CI_FAILED_STEPS.txt.'
fi
printf 'PASS all-platform GitHub build\n'

printf '[4/8] Download and verify every distributable artifact...\n'
cd "$ROOT"
OUT="$ROOT/READY_TO_PUBLISH"
rm -rf "$OUT"; mkdir -p "$OUT"
gh run download "$RUN_ID" --repo "$REPO_SLUG" --dir "$OUT" || fail 'Could not download GitHub build artifacts.'
python3 scripts/verify_all_platform_artifacts.py "$OUT" || fail 'Downloaded artifact verification failed.'
python3 scripts/publish_marketplace.py --version "$VERSION" --dir "$OUT" --verify-only || fail 'Marketplace version/target preflight failed.'
printf 'PASS 9 VSIX packages + 9 standalone language-server archives\n'

printf '[5/8] Everything is ready; no public release has been made yet.\n'
printf 'GitHub: %s\nRust: core-1o %s + language-server-1o %s\nMarketplace: 1o.1o %s across 9 targets\n' "$REPO_SLUG" "$CORE_VERSION" "$SERVER_VERSION" "$VERSION"
read -r -p 'Type PUBLISH to publish ALL of it, or press Enter to stop: ' ANSWER
if [ "$ANSWER" != 'PUBLISH' ]; then
  printf '\nREADY, NOT PUBLISHED. Artifacts: %s\n' "$OUT"
  exit 0
fi

printf '[6/8] Make this tested source the canonical GitHub source...\n'
cd "$WORK/repo"
git fetch origin "$DEFAULT" >/dev/null 2>&1 || fail 'Could not refresh the default branch.'
CURRENT_DEFAULT="$(git rev-parse "origin/$DEFAULT")"
[ "$CURRENT_DEFAULT" = "$BASE_SHA" ] || fail 'The GitHub default branch changed while tests ran. Rerun this command so the new source is included safely.'
if git push origin "$BRANCH:$DEFAULT" >/dev/null 2>&1; then
  printf 'PASS GitHub %s updated directly\n' "$DEFAULT"
else
  printf 'INFO Direct update is protected; using an automated pull request.\n'
  PR_URL="$(gh pr create --repo "$REPO_SLUG" --base "$DEFAULT" --head "$BRANCH" --title "Release 1o $VERSION" --body "Automated all-platform-qualified 1o $VERSION release source." 2>/dev/null || true)"
  if [ -z "$PR_URL" ]; then
    PR_URL="$(gh pr list --repo "$REPO_SLUG" --head "$BRANCH" --base "$DEFAULT" --json url --jq '.[0].url' 2>/dev/null || true)"
  fi
  [ -n "$PR_URL" ] || fail 'Could not create the protected-branch release pull request.'
  gh pr merge "$PR_URL" --repo "$REPO_SLUG" --merge --delete-branch >/dev/null 2>&1 || fail 'GitHub branch protection did not allow automatic merge. No crates or Marketplace packages were published.'
  printf 'PASS GitHub source merged\n'
fi

printf '[7/8] Publish the two Rust packages and all Marketplace variants...\n'
cd "$ROOT"
ONEO_MASTER_RELEASE_CONFIRMED=YES python3 scripts/publish_crates.py --publish-confirmed || fail 'Rust publication failed; inspect CRATES_RELEASE_RESULT.txt before retrying.'
python3 scripts/publish_marketplace.py --version "$VERSION" --dir "$OUT" || fail 'Marketplace publication failed; rerunning this command can safely resume confirmed targets.'
printf 'PASS crates.io + VS Code Marketplace publication\n'

printf '[8/8] Create the GitHub Release with binaries, VSIX files and checksums...\n'
ASSETS=()
while IFS= read -r f; do ASSETS+=("$f"); done < <(find "$OUT" -type f \( -name '*.vsix' -o -name '*.zip' -o -name '*.tar.gz' -o -name 'SHA256SUMS.txt' \) | sort)
[ "${#ASSETS[@]}" -ge 19 ] || fail 'Release artifact set is incomplete.'
gh release create "$TAG" "${ASSETS[@]}" --repo "$REPO_SLUG" --target "$DEFAULT" --title "1o $VERSION" --notes "1o $VERSION: core-1o $CORE_VERSION, language-server-1o $SERVER_VERSION, and the 1o.1o extension for macOS, Windows, Linux and Alpine." || fail 'Could not create the GitHub Release.'
git push origin --delete "$BRANCH" >/dev/null 2>&1 || true

printf '\n1o RELEASE COMPLETE\nGitHub source: %s (%s)\nGitHub release: %s\ncrates.io: core-1o %s + language-server-1o %s\nVS Code Marketplace: 1o.1o %s (9 targets)\nReport: %s\n' "$REPO_SLUG" "$DEFAULT" "$TAG" "$CORE_VERSION" "$SERVER_VERSION" "$VERSION" "$LOG"
