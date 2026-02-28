# Maintainer: Harlen Batagelo <hbatagelo@gmail.com>
pkgname=shaderbg
pkgver=1.2.0
pkgrel=1
pkgdesc="Shader wallpaper utility for Wayland"
arch=('x86_64')
url="https://github.com/hbatagelo/shaderbg"
license=('GPL-3.0-or-later')
depends=('gtk4' 'gtk4-layer-shell')
makedepends=('cargo' 'pandoc' 'groff')
options=('!debug')

build() {
    cd "$srcdir"

    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target

    cargo build --release --locked --features generate-manpage
}

check() {
    cd "$srcdir"
    cargo test --release --locked
}

package() {
    cd "$srcdir"

    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
    install -Dm644 "doc/$pkgname.1" "$pkgdir/usr/share/man/man1/$pkgname.1"
    install -dm755 "$pkgdir/usr/share/$pkgname/assets"
    install -dm755 "$pkgdir/usr/share/$pkgname/assets/cubemaps"
    install -Dm644 data/assets/cubemaps/* "$pkgdir/usr/share/$pkgname/assets/cubemaps/"
    install -dm755 "$pkgdir/usr/share/$pkgname/assets/textures"
    install -Dm644 data/assets/textures/* "$pkgdir/usr/share/$pkgname/assets/textures/"
    install -dm755 "$pkgdir/usr/share/$pkgname/assets/volumes"
    install -Dm644 data/assets/volumes/* "$pkgdir/usr/share/$pkgname/assets/volumes/"
    install -dm755 "$pkgdir/usr/share/$pkgname/presets"
    install -Dm644 data/presets/* "$pkgdir/usr/share/$pkgname/presets/"
    install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}