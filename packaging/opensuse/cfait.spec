Name:           cfait
Version:        1.1.6
Release:        0
Summary:        Offline-first task manager with CalDAV sync (TUI and GUI)
License:        GPL-3.0-or-later
URL:            https://git.disroot.org/trougnouf/cfait
Source0:        %{name}-%{version}.tar.zst
Source1:        vendor.tar.zst
BuildRequires:  cargo-packaging
BuildRequires:  pkgconfig(fontconfig)
BuildRequires:  pkgconfig(xkbcommon)
BuildRequires:  pkgconfig(vulkan)
Requires:       libsecret-0
Recommends:     vulkan-driver
ExclusiveArch:  %{rust_tier1_arches}

%description
Cfait is an offline-first task manager that synchronizes with standard CalDAV
servers. It provides a keyboard-centric terminal interface (TUI) and a desktop
graphical interface (GUI), both backed by a shared Rust core.

%prep
%autosetup -p1 -a1

%build
export AWS_LC_SYS_NO_JITTER_ENTROPY=1
%cargo_build --features gui

%install
install -D -d -m 0755 %{buildroot}%{_bindir}
install -m 0755 %{_builddir}/%{name}-%{version}/target/release/cfait %{buildroot}%{_bindir}/cfait
install -m 0755 %{_builddir}/%{name}-%{version}/target/release/cfait-gui %{buildroot}%{_bindir}/cfait-gui

install -D -m 644 assets/cfait.desktop %{buildroot}%{_datadir}/applications/cfait.desktop
install -D -m 644 assets/cfait.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/cfait.svg

%check
%cargo_test --features gui

%files
%{_bindir}/cfait
%{_bindir}/cfait-gui
%{_datadir}/applications/cfait.desktop
%{_datadir}/icons/hicolor/scalable/apps/cfait.svg
%license LICENSE
%doc README.md

%changelog
