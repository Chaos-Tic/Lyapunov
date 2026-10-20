# Builds Lyapunov from the release tarball. With vendor (the default, for
# COPR) the crates come from the vendor archive attached to the release;
# built --without vendor, they come from Fedora's rust-* packages.
%bcond vendor 1

Name:           lyapunov
Version:        0.2.0
Release:        1%{?dist}
Summary:        Save an activity as a card and resume it later

SourceLicense:  GPL-3.0-or-later
# Lyapunov is GPL-3.0-or-later; the rest covers the crates linked into lyapunov.
License:        GPL-3.0-or-later AND MIT AND (MIT OR Apache-2.0) AND Unicode-3.0 AND (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT)
URL:            https://github.com/Chaos-Tic/Lyapunov
Source0:        %{url}/archive/v%{version}/Lyapunov-%{version}.tar.gz
%if %{with vendor}
# packaging/linux/vendor.sh
Source1:        %{url}/releases/download/v%{version}/lyapunov-%{version}-vendor.tar.xz
%endif

BuildRequires:  cargo-rpm-macros >= 25
BuildRequires:  cmake
BuildRequires:  gcc-c++
BuildRequires:  cmake(Qt6Widgets)
BuildRequires:  cmake(Qt6Network)
BuildRequires:  desktop-file-utils

Requires:       qt6-qtbase-gui
Recommends:     xdg-utils
Recommends:     qt6-qtwayland

%description
Lyapunov keeps each activity as a card: the applications, windows, folders
and documents that belong to it. Resuming a card reopens what is missing and
puts windows back where they were, on KDE Plasma, GNOME (with the included
Shell extension), X11 window managers, sway, i3 and Hyprland. Everything stays
on this computer.

%prep
%if %{with vendor}
%autosetup -n Lyapunov-%{version} -p1 -a1
%cargo_prep -v vendor
%else
%autosetup -n Lyapunov-%{version} -p1
%cargo_prep
%endif

%if %{without vendor}
%generate_buildrequires
%cargo_generate_buildrequires
%endif

%build
%cargo_build
%{cargo_license_summary}
%{cargo_license} > LICENSE.dependencies
%if %{with vendor}
%cargo_vendor_manifest
%endif
%cmake -DBUILD_TESTING=OFF -DLYAPUNOV_CLI_BINARY="$PWD/target/rpm/lyapunov"
%cmake_build

%install
%cmake_install

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/lyapunov.desktop
%cargo_test

%files
%license %{_datadir}/licenses/lyapunov/LICENSE
%license LICENSE.dependencies
%if %{with vendor}
%license cargo-vendor.txt
%endif
%doc README.md
%{_bindir}/lyapunov
%{_bindir}/lyapunov-desktop
%{_datadir}/applications/lyapunov.desktop
%{_datadir}/icons/hicolor/scalable/apps/lyapunov.svg
%{_datadir}/icons/hicolor/symbolic/apps/lyapunov-symbolic.svg
# GNOME Shell reads this; it is not required.
%dir %{_datadir}/gnome-shell
%dir %{_datadir}/gnome-shell/extensions
%{_datadir}/gnome-shell/extensions/lyapunov@chaos-tic.github.io

%changelog
* Tue Oct 20 2026 Chaos-Tic <58691435+Chaos-Tic@users.noreply.github.com> - 0.2.0-1
- Offer updates, new icon, standard CMake install rules

* Tue Oct 20 2026 Chaos-Tic <58691435+Chaos-Tic@users.noreply.github.com> - 0.1.0-1
- First package built from source
