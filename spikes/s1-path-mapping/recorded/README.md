# Init packets used by the unit tests

`linux-php85-public.xml` and `linux-php83-root.xml` were recorded from the
test bed (PHP 8.5.12 and 8.3.36, Xdebug 3.5.3) with `s1-path-mapping accept`.

The other files are the PHP 8.5 recording with only `fileuri` replaced.
No Windows or macOS host was available, so each value is the URI that
Xdebug 3.5.3 builds in `xdebug_path_to_url` (`src/lib/usefulstuff.c`) for
that platform:

| File | Platform | `fileuri` |
|---|---|---|
| `windows-native-drive.xml` | PHP on Windows, drive path with a space | `file:///C:/laragon/www/My%20Shop/public/index.php` |
| `windows-native-unc.xml` | PHP on Windows, code on a UNC share | `file://fileserver/www/shop/index.php` |
| `wsl-php.xml` | PHP inside WSL, IDE on Windows | `file:///home/dev/shop/public/index.php` |
| `macos-docker.xml` | Docker on macOS, folder named `Public` | `file:///var/www/html/Public/index.php` |
