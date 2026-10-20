# Verify a release download

[Back to the README](../README.md)

Download the installer or package and `SHA256SUMS` from the same
[GitHub release](https://github.com/Chaos-Tic/Lyapunov/releases). In their
download directory, run:

```sh
sha256sum -c --ignore-missing SHA256SUMS
```

On Windows, compare the installer hash with its line in `SHA256SUMS`:

```powershell
(Get-FileHash .\Lyapunov-Setup-x64.exe -Algorithm SHA256).Hash
```

The hash checks whether the downloaded file matches the checksum in that
release. It is not a code signature. The Windows installer is not signed by
a trusted publisher, so Windows may show a SmartScreen warning. Review the
release and its source before deciding whether to run it; do not disable
SmartScreen system-wide.
