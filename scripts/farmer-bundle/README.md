# AGBot farmer app

This bundle starts the browser application on your computer. It needs Docker
with Compose running and an internet connection to download the application
image and satellite imagery. Git, Rust, and a separate GIS server are not needed.

The published application image currently targets Linux x86-64 (amd64).
Intel Macs can run it through Docker Desktop. Apple Silicon requires container
emulation; that configuration has not yet been validated. There is no native
ARM64 image or Windows launcher in this bundle.

## Start

1. Extract the archive and keep its files together.
2. Start Docker Desktop on a Mac, or Docker Engine with Compose on Linux.
3. On a Mac, open **Start AGBot.command**. On Linux, run `./start.sh` from
   the extracted folder.
4. Open the portal URL printed in the terminal, normally
   `http://127.0.0.1:8080/portal`, and enter the displayed access code.
5. Add your farm and field boundaries in the application.

The first download can take time. If a release image is unavailable, startup
stops with an error. It never changes to another release channel automatically.
`CHANNEL` identifies the release channel this launcher follows; `VERSION`
identifies this launcher bundle. Startup pulls the latest image in that channel,
so the image may be newer than the launcher bundle.

## Return later

Start the app using the same launcher. Your fields and files remain in Docker
volumes when it stops. To stop it, open **Stop AGBot.command** on a Mac, or run
`./stop.sh` on Linux. Do not delete AGBot's Docker volumes unless you intend to
erase your farm data.

If you already installed AGBot from a repository checkout, keep using that
installation until its database and imagery volumes have been migrated and
verified. This bundle uses Docker Compose project name `agbot`; an older
installation may use different volume names. Extracting this bundle does not
migrate or reset existing farm data.

Show your first-login code again by running `./scripts/agbot access-code` in
this folder. The code is stored privately at `~/.agbot/bootstrap-access-code`.
An existing database retains its original account code; creating a new code
file does not reset that account. If you provide `AGBOT_BOOTSTRAP_ACCESS_CODE`
yourself, preserve that value for subsequent use.

The app is accessible only from this computer by default. Internet publishing
and access from other devices require separate network and authentication setup.

## Verify the download

Keep the `.sha256` file beside the downloaded archive and run:

```sh
shasum -a 256 -c agbot-VERSION.sha256
```

Replace `VERSION` with the version in the downloaded filename. On Linux,
`sha256sum -c` can also check the file.
