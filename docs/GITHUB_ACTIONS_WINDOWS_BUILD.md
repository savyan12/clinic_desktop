# GitHub Actions Windows NSIS builds

The repository-root workflow is [`.github/workflows/windows-build.yml`](../.github/workflows/windows-build.yml). It builds the existing offline desktop edition for **Windows x64**, using the existing NSIS configuration. It does not build an MSI.

A successful Actions run produces a package and verifies its build inputs. **It does not certify installation or operation on Windows 10 or Windows 11.** Complete the separate clean-Windows acceptance process below before customer delivery. No clean-Windows test result is implied by this document.

## Source layout and build scope

The desktop application has its own Vue source/package lock and embedded Rust backend:

```text
.github/workflows/windows-build.yml
package.json
package-lock.json
scripts/windows-runtime.lock.json
src-tauri/Cargo.lock
src-tauri/tauri.conf.json
```

This repository contains only the desktop application. The retained `backend/` directory is a migration reference and is neither staged nor packaged.

Application release versions come from `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`; both must agree. The frontend package's `0.0.0` is not the installer version.

## Trigger a build

First commit/push the workflow and its supporting scripts. The manual workflow must exist on the repository's default branch before GitHub displays the **Run workflow** button.

- **Manual:** open `savyan12/clinic_desktop` → **Actions** → **Windows NSIS build** → **Run workflow**, then select the desired branch or tag. Manual runs upload an Actions artifact only, including manual runs against a tag.
- **Main branch:** every push to `main` builds the standalone desktop repository. Ordinary pushes do not publish a GitHub Release.
- **Release:** push a new `v*` tag. GitHub does not apply push path filters to tag pushes. The tag must exactly match the application version, for example `v1.0.0` for application version `1.0.0`.

After updating and committing both application version files, use a new, unused version tag:

```bash
git tag v1.0.0
git push origin v1.0.0
```

Use the appropriate new version instead of reusing an existing published tag. A mismatched tag fails before dependency downloads. New main/manual runs can cancel an older run on the same branch; tag runs are not cancelled in progress by this workflow.

## Download locations

For every successful run, open the run's **Artifacts** section and download **`clinic-system-windows-x64`**. Retention is 30 days, subject to repository storage/retention policy. Extract the artifact ZIP before giving the installer to a customer.

The build command is the existing script, with CI and Cargo lock enforcement:

```powershell
npm run desktop:build:windows -- --ci -- --locked
```

It runs `tauri build --bundles nsis`. The x64 MSVC host is selected without an explicit cross-compilation output directory, so the original installer is at:

```text
src-tauri/target/release/bundle/nsis/*.exe
```

The workflow requires exactly one installer. It collects a copy with an ASCII delivery filename, without changing the installed application name:

```text
release-artifacts/
    Clinic-System_1.0.0_x64-setup.exe
    Clinic-System_1.0.0_x64-setup.exe.sha256
    runtime-versions.json
```

The actual application version replaces `1.0.0`. The checksum file uses the standard `SHA256  filename` format. `runtime-versions.json` records the application commit, installer hash, exact runtime versions, official download URLs, and archive/installer hashes.

For successful tag pushes, all three files are also attached under the repository's **Releases** page. The official `actions/github-script` action calls GitHub's Releases API. It creates a draft, uploads all assets, then publishes. An upload failure leaves a draft for retry. A published release's assets are never silently replaced: rerunning its release job accepts identical assets; a different build requires a new version/tag.

## Repository settings and secrets

- Enable GitHub Actions and allow the pinned official `actions/checkout`, `setup-node`, `cache`, `upload-artifact`, `download-artifact`, and `github-script` actions.
- Allow GitHub-hosted `windows-2022` runners and sufficient Actions/artifact storage. A release uses a separate `ubuntu-24.04` job for uploading files only.
- Repository/organization policy must permit the release job's **`contents: write`** permission. The build has only **`contents: read`**, checkout credentials are not persisted, and the write permission is restricted to the successful tag-release job.
- **No repository secrets, PAT, production `.env`, database credentials, or signing certificate are required.** The release job uses the automatically supplied `GITHUB_TOKEN`.
- Protect `main` and release tags according to your repository's maintainer policy. This workflow has no `pull_request_target` trigger and does not publish releases from ordinary branch builds.

Application code signing is not configured by this change. Hashes verify file integrity, not publisher identity. Keep Windows security checks enabled; follow the [customer installation guide](CUSTOMER_INSTALLATION.md) when evaluating an unsigned installer.

## Controlled versions and provenance

Runtime pins and SHA-256 values are stored in [`scripts/windows-runtime.lock.json`](../scripts/windows-runtime.lock.json). They are verified before archive extraction or execution.

| Component | Version / policy | Customer installation |
| --- | --- | --- |
| MySQL Community Server | **8.4.13 LTS**, official x64 ZIP, including all client/backup tools | Embedded |
| Microsoft Visual C++ x64 Redistributable | **14.44.35211.0** | Official installer embedded |
| WebView2 | Official **x64 Evergreen offline installer**, exact version and SHA-256 captured per build | Embedded by Tauri's existing `offlineInstaller` mode |
| Node.js | **24.16.0**; compatible with Vite's `^20.19.0 || >=22.12.0` requirement | Build machine only |
| Rust | **1.99.1 stable**, `x86_64-pc-windows-msvc` | Build machine only |
| Tauri CLI / Rust Tauri | **2.12.4 / 2.12.5**, from the npm/Cargo locks | Compiled application |
| NSIS | **3.13**, downloaded and checksum-checked by the locked Tauri bundler | Installer technology |
| C++ tools and SDK | Visual Studio **2022** (17.x) plus the Windows SDK supplied by `windows-2022`, checked before building | Build machine only |

GitHub Actions are pinned to full commit hashes, with release versions in comments. The runner image and Microsoft's Evergreen WebView2 payload can change over time. In particular, the existing Tauri `offlineInstaller` configuration has no version pin: the workflow resolves the same official URL as Tauri, verifies Microsoft's Authenticode signature, and checks that the rendered NSIS script embeds **that exact validated file**. If Microsoft rotates it between preparation and packaging, the run fails and must be retried. This preserves the current offline installer behavior without pretending the build is byte-for-byte reproducible.

Official source references:

- MySQL [8.4 downloads](https://dev.mysql.com/downloads/mysql/8.4.html), [archive signature](https://dev.mysql.com/downloads/gpg/?file=mysql-8.4.13-winx64.zip&p=23), and [Oracle signing key](https://repo.mysql.com/RPM-GPG-KEY-mysql-2025). The pinned archive's Oracle signature was verified with fingerprint `BCA43417C3B485DD128EC6D4B7B3B788A8D3785C` before recording its SHA-256; CI checks that SHA-256.
- Microsoft [VC++ redistributable documentation](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) and [WebView2 distribution](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution). VC++ uses an immutable Microsoft download URL, a pinned SHA-256, and Authenticode validation; WebView2 also requires a valid Microsoft signature.
- The locked [Tauri NSIS bundler](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs) supplies its official toolchain; no Chocolatey NSIS override, WiX, or MSI migration is needed.

To update a runtime, obtain its official archive/checksum or verify its official signature, update the lock entry and these version notes, then run CI and repeat clean-Windows acceptance. Do not copy a hash from an untrusted mirror or update a hash merely to silence a mismatch. MySQL changes also require testing startup and backup restore from the previous delivered version.

## Build preparation and checks

The workflow:

1. Checks out the source, verifies Tauri/Cargo/tag versions and the preinstalled x64 C++ tools/SDK, selects the pinned Rust toolchain, and configures npm/Cargo caches.
2. Runs **`npm ci`** at the repository root, followed by **`npm run lint`**, **`npm run verify:ui`**, runtime-staging tests, and **`npm run build`**.
3. Downloads the official MySQL and VC++ packages to runner-temporary storage, verifies pinned SHA-256 values and Microsoft signatures, and authenticates the exact WebView2 payload Tauri will embed.
4. Calls **`prepare-windows-runtime.ps1`** and verifies the MySQL x64 executables, version, backup tools, VC++ installer, and Tauri resource allowlist. PHP, Composer, Laravel, and the retained backend directory are rejected from release resources.
5. Runs **`cargo fmt --check`**, **`cargo check --locked`**, and **`cargo test --locked`**, then the NSIS build. The collector inspects the rendered resource list, x64 architecture, and offline WebView2 payload before calculating the installer SHA-256.

The normal CI Rust suite does not start the bundled Windows MySQL service, exercise installation, perform a Windows restart, or use a physical USB drive. Those remain acceptance items below. Database, API lifecycle, financial, schema-compatibility, and backup/restore integration tests are run locally against isolated MySQL environments during development.

For manual Windows release builds, use the [customer guide's release-owner appendix](CUSTOMER_INSTALLATION.md#10-release-owner-appendix-prepare-the-windows-installer). Node.js and Rust are build-time tools only; PHP and Composer are not required.

## Separate Windows 10 and Windows 11 acceptance

`windows-2022` is a Windows **Server** build host with development tools. It is not either customer OS and is not a clean customer environment. There is no automated claim of Windows 10/11 compatibility in this workflow.

Test the **same downloaded installer hash** on two independent, disposable environments:

- **Windows 10 x64**, record edition, 22H2/build number, updates and servicing/ESU status.
- **Windows 11 x64**, record the exact supported edition, feature release, build number, and updates.

Use clean VM snapshots or physical PCs with no MySQL service, Node, Rust, Visual Studio, or clinic data. Use a normal daily Windows account and an authorized administrator for prerequisite elevation. Include a username/install path containing spaces and Arabic characters. Use dummy clinic data and keep the original installer, checksum, runtime manifest, and OS build details with the results.

Disconnect networking **before launching the installer**, not only after activation. Use a disposable image prepared with no VC++/WebView2, where possible, to prove the embedded installers work; also test an image with equal/newer prerequisites. Do not remove prerequisites from a customer's live PC to create a test fixture. If a missing-prerequisite condition was not actually tested, record it as not tested.

| Test and required evidence | Windows 10 x64 | Windows 11 x64 |
| --- | --- | --- |
| Fresh offline NSIS installation under the daily user; no developer tools or downloads needed | Not run | Not run |
| Missing VC++ installs with elevation; denied elevation/failure gives a clear error and no success claim | Not run | Not run |
| Equal/newer x64 VC++ is accepted; older version upgrades; restart-required outcome is handled | Not run | Not run |
| Missing WebView2 installs offline; existing WebView2 works; prerequisite failures are reported | Not run | Not run |
| Fresh activation creates one admin and empty clinic data without network access | Not run | Not run |
| Embedded Rust API starts, health/login work, and no PHP/Composer installation is present or required | Not run | Not run |
| Bundled MySQL initializes its private data directory and MySQL/Rust API listen only on loopback | Not run | Not run |
| Admin/reception workflows and Arabic PDF/Excel/printing work on dummy records | Not run | Not run |
| Normal shutdown releases app-owned Rust API/MySQL processes and ports; restart preserves records | Not run | Not run |
| Windows reboot followed by login/start preserves patients, session totals, debts, and wallet balances | Not run | Not run |
| A real removable USB backup is verified and contains SQL, manifest, and completion checksum | Not run | Not run |
| First-activation restore on a second clean installation succeeds; records and financial totals match | Not run | Not run |
| Damaged/incomplete backup is rejected; existing accepted backups remain intact | Not run | Not run |
| Upgrade from the previous delivered app/runtime preserves data and accepts its verified backup | Not run | Not run |

Replace each status only with actual evidence: tester/date, OS/build, installer SHA-256, prerequisite versions, steps, expected/actual results, and redacted logs/screenshots. Test USB passthrough/removable-drive classification if using VMs. An ordinary host folder is not a Windows removable USB test.

Use the existing [installation, activation, backup and restore procedures](CUSTOMER_INSTALLATION.md). Restore is available during first activation; do not invent an in-place Settings restore or delete installation data to expose that screen. Do not approve customer delivery while any required install/runtime/offline/data-persistence/restore scenario is failing or untested.

## Troubleshooting

| Failure | Action |
| --- | --- |
| Manual workflow is absent | Commit repository-root `.github/workflows/windows-build.yml` on the default branch and enable Actions. |
| Tag/version mismatch | Commit matching Tauri/Cargo versions, then create a new matching tag. Do not retag a published release. |
| `npm ci` fails | Reconcile `package.json` and its lockfile in a development change; do not replace `npm ci` with an unlocked install in CI. |
| Archive hash/signature mismatch | Stop. Verify the vendor release and URL independently. Never accept the new hash automatically, skip certificate checks, or download replacement DLLs from unofficial sites. |
| Vendor download unavailable | The MySQL downloader tries the same pinned archive in the vendor's archive directory. Retry a transient outage; review/update pins if the official archive has been withdrawn. |
| VC++ failure | Check the x64 runtime registry version, elevation, exit status and pending reboot. The installer accepts `0`, marks `3010` as reboot required, and accepts `1638` only after verifying a compatible x64 runtime. Customer failures identify `%TEMP%\clinic-vcredist-install.log`. A build-runner reboot requirement fails explicitly. |
| C++ tools/Windows SDK missing | Verify `windows-2022` image availability and VS 2022 C++ workload/SDK contents. Restore supported runner prerequisites; do not switch the customer package to MSI. |
| WebView2 payload changed during build | Rerun to authenticate Microsoft's new payload. The artifact collector deliberately rejects an unvalidated replacement. A changed redirect layout requires reviewing the official Tauri/Microsoft download behavior. |
| NSIS/tool download error or no `.exe` | Inspect the Tauri log and `target/release/nsis/x64/installer.nsi`. Keep `--bundles nsis`. The collector fails on zero or multiple installers; it never falls back to `.msi`. |
| Staged credential/data/cache file found | Use a clean checkout and the staging script. Do not put customer data in the source tree or manually whitelist a secret. |
| Release permission/upload error | Check Actions policy permits the release job's `contents: write`, artifact retention, and tag rules. Retry the failed release job with its existing artifact; complete uploads before publishing its draft. |
| Published release differs on rerun | Use a new version/tag. Published binaries are intentionally immutable; do not delete working releases to make a rebuild pass. |

Build logs and the run summary identify versions and hashes. Do not upload patient databases, private keys, `.env`, `install.json`, or `mysql-root.cnf` as diagnostic artifacts. The workflow uploads only the distributable installer and its verification files.
