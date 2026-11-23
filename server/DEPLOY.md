# Deploying the account server on a VPS

This guide takes you from an empty VPS to a working account server at an address like
`https://accounts.example.org`. It then shows how to build the app so it uses that address.

The commands are for **Ubuntu 22.04/24.04 or Debian 12**. Replace these placeholders everywhere:

| Placeholder | Replace with |
|---|---|
| `accounts.example.org` | your domain or subdomain for the server |
| `you@vps` | your SSH login on the VPS |
| `203.0.113.10` | your VPS's IP address |

---

## 0. What you need

- **A VPS.** The smallest plan is enough: 1 vCPU and 512 MB RAM. The server uses a few MB of memory and one SQLite file.
- **A domain name** you control, so you can add a DNS record.
- **An email sending service (SMTP)** for the 6-digit codes. Any provider works, for example Brevo, Mailgun, Amazon SES, Postmark, Zoho, or your domain host's mailbox. You need its host, port, username and password.
- **Optional:** a Google Cloud account for "Continue with Google" (step 9).

## 1. Point the domain at the VPS

In your DNS provider, add these records:

```
accounts.example.org.   A      203.0.113.10
accounts.example.org.   AAAA   <IPv6 address>      (only if your VPS has one)
```

Wait until it resolves. This can take from a minute to an hour:

```sh
dig +short accounts.example.org      # should print 203.0.113.10
```

## 2. Prepare the VPS

```sh
ssh you@vps
sudo apt update && sudo apt upgrade -y
sudo apt install -y build-essential pkg-config curl git sqlite3 ufw

# firewall: only SSH and web traffic
sudo ufw allow OpenSSH
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw enable
```

Port 5764 stays closed. The server listens only on `127.0.0.1`, and Caddy (step 6) is the only way in.

Create a system user that will run the server and own its data:

```sh
sudo useradd --system --create-home --home-dir /var/lib/study-library --shell /usr/sbin/nologin studylib
```

## 3. Copy the server code to the VPS

Pick one of these.

**A. From your computer with rsync.** No git hosting is needed. Run this on your computer, from the project folder:

```sh
rsync -av --exclude target --exclude '*.db*' server/ you@vps:~/study-library-server/
```

**B. With git**, if the repository is on GitHub or similar. On the VPS:

```sh
git clone https://github.com/<you>/<repo>.git ~/study-library
ln -s ~/study-library/server ~/study-library-server
```

## 3½. Alternative: run it with Docker (replaces steps 4–7)

If you prefer Docker, skip steps 4–7 and do this instead, then continue with step 8.
`docker-compose.yml` runs two containers: the server, and Caddy in front of it for HTTPS.
Caddy gets the Let's Encrypt certificate on first start and renews it by itself.
You don't need Rust, the `studylib` user, the systemd unit or a Caddy install on the host.
If Caddy is already installed on the host, stop it first (`sudo systemctl disable --now caddy`), because both need ports 80 and 443.

Install Docker (official convenience script):

```sh
curl -fsSL https://get.docker.com | sudo sh
```

Create the two settings files next to `docker-compose.yml`:

```sh
cd ~/study-library-server
cp server.env.example server.env && chmod 600 server.env
nano server.env            # SMTP_* and GOOGLE_*; see the notes in step 5
cp .env.example .env
nano .env                  # DOMAIN=accounts.example.org, or 92-5-32-249.sslip.io without a domain
```

Ports 80 and 443 must be open, including in the VPS provider's firewall panel. Docker publishes them past `ufw`.

Build and start it:

```sh
sudo docker compose up -d --build                 # first build takes a few minutes
sudo docker compose ps                            # both containers "Up"; the server "healthy" after ~30 s
curl -s https://accounts.example.org/v1/config    # → {"google_client_id":null,"version":1}
sudo docker compose logs -f                       # logs of both; codes appear here while email is not configured
```

How it fits together:

- **Ports:** only Caddy is public (80 and 443). The server's plain-HTTP port 5764 is published only on the VPS's `127.0.0.1`, for checks with `curl http://127.0.0.1:5764/v1/config`.
- **Certificates:** stored in the `caddy-data` volume, so restarts and rebuilds don't request new ones. Renewal happens automatically about 30 days before expiry; nothing to schedule.
- **Data:** the SQLite database lives in the volume `study-library-data`, mounted at `/data`. It survives rebuilds and restarts. `docker compose down -v` deletes all volumes, so never use `-v`.
- **Restart:** the containers start again on reboot and after a crash (`restart: unless-stopped`).
- **Updating:** copy the new code (step 3), then `sudo docker compose up -d --build`.
- **Backup:** don't copy the database file while the server runs. Use SQLite's own backup instead, which writes `accounts-backup.db` into the current folder:
  `sudo docker run --rm -v study-library-server_study-library-data:/data -v "$PWD":/out debian:bookworm-slim sh -c 'apt-get update -qq && apt-get install -y -qq sqlite3 >/dev/null && sqlite3 /data/accounts.db ".backup /out/accounts-backup.db"'`

## 4. Build it on the VPS

Install Rust for your login user, not root:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
```

Build and install the program:

```sh
cd ~/study-library-server
cargo build --release          # first build takes a few minutes
sudo install -m 755 target/release/study-library-server /usr/local/bin/study-library-server
```

> **Alternative: build on your computer and copy only the program.** Your computer's Linux is
> probably newer than the VPS's, so build against an old glibc with the same tool the app installer uses:
> `cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.31` (use `aarch64-…` for ARM VPSes).
> Then `scp target/x86_64-unknown-linux-gnu/release/study-library-server you@vps:` and install it as above.

## 5. Configure it

Create the settings file. It holds passwords, so only root can read it:

```sh
sudo mkdir -p /etc/study-library
sudo nano /etc/study-library/server.env
```

```ini
BIND=127.0.0.1:5764
DB_PATH=/var/lib/study-library/accounts.db

# Email for the confirmation / password-reset codes
SMTP_HOST=smtp.your-provider.com
SMTP_PORT=587
SMTP_USER=your-smtp-username
SMTP_PASS=your-smtp-password
SMTP_FROM=Study Library <no-reply@example.org>

# Google sign-in (optional, see step 9). Leave empty to switch it off.
GOOGLE_CLIENT_ID=
GOOGLE_CLIENT_SECRET=
```

```sh
sudo chmod 600 /etc/study-library/server.env
```

Notes on the email settings:

- **`SMTP_PORT`:** 587 is the common choice and uses STARTTLS. Use 465 if your provider says "SSL/TLS".
- **`SMTP_FROM`:** must be an address your provider allows you to send from.
- **Avoid the spam folder:** add the SPF and DKIM DNS records your email provider gives you. Otherwise the codes often land in spam.
- **Testing without email:** while `SMTP_HOST` is empty, the server prints each code to its log instead of emailing it.

## 6. Run it as a service (systemd)

```sh
sudo nano /etc/systemd/system/study-library-server.service
```

```ini
[Unit]
Description=Study Library account server
After=network-online.target
Wants=network-online.target

[Service]
User=studylib
Group=studylib
EnvironmentFile=/etc/study-library/server.env
WorkingDirectory=/var/lib/study-library
ExecStart=/usr/local/bin/study-library-server
Restart=on-failure
RestartSec=3

# hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/study-library

[Install]
WantedBy=multi-user.target
```

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now study-library-server
sudo systemctl status study-library-server        # should say "active (running)"
curl -s http://127.0.0.1:5764/v1/config            # → {"google_client_id":null,"version":1}
```

To watch the logs, which also show codes while email is not configured:

```sh
sudo journalctl -u study-library-server -f
```

## 7. HTTPS with Caddy

Caddy gets and renews the HTTPS certificate automatically.

```sh
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update && sudo apt install -y caddy
```

Replace the contents of `/etc/caddy/Caddyfile` with:

```
accounts.example.org {
    encode gzip
    reverse_proxy 127.0.0.1:5764
}
```

```sh
sudo systemctl reload caddy
```

Check it from **your own computer**:

```sh
curl https://accounts.example.org/v1/config
```

It should print the same JSON as in step 6. If it fails, see "Troubleshooting" at the end.

## 8. Build the app with your server address

The address is built into the app, and the Settings page does not show it. To set it:

1. Open **`app/account.conf`** in this project and set:

   ```
   server_url = "https://accounts.example.org"
   ```

2. Rebuild the app as usual:

   ```sh
   cd app
   cargo build --release            # or: ./packaging/make-installer.sh for the installer
   ```

`app/build.rs` reads `account.conf` on every build and rebuilds when the file changes.

- **One-off builds:** you can override the file with an environment variable, which is useful for testing against another server:

  ```sh
  STUDY_LIBRARY_SERVER=https://staging.example.org cargo build --release
  ```

- **HTTP warning:** the build prints a warning if the address is not `https://`. Plain HTTP is only acceptable for `127.0.0.1` during development.
- **Changing the address later:** people signed in to the old address are signed out on the next start, because their session belongs to the other server. Their notes stay on their computer, and they sign in again.

## 9. Optional: Google sign-in

1. Go to <https://console.cloud.google.com/> and create a project, for example "Study Library".
2. Open **APIs & Services → OAuth consent screen**:
   - User type **External**. Fill in the app name, support email and developer email.
   - Scopes: `openid`, `.../auth/userinfo.email`, `.../auth/userinfo.profile`.
   - While the app is in **Testing**, only the test users you list can sign in. Press **Publish app** to open it to everyone. Basic scopes like these don't need Google's review.
3. Open **APIs & Services → Credentials → Create credentials → OAuth client ID**:
   - Application type: **Desktop app**. Name: "Study Library".
   - Copy the **Client ID** and **Client secret**.
4. Put them in `/etc/study-library/server.env`:

   ```ini
   GOOGLE_CLIENT_ID=1234567890-abc...apps.googleusercontent.com
   GOOGLE_CLIENT_SECRET=GOCSPX-...
   ```

5. Restart the server, then check that the client ID now appears:

   ```sh
   sudo systemctl restart study-library-server
   curl https://accounts.example.org/v1/config
   ```

The app asks the server for this on its own. Once the client ID shows up, "Continue with Google" appears in the sign-in window without rebuilding the app. The client secret stays on the server and is never put in the app.

Apple sign-in is not built yet.

## 10. Check the whole flow

1. Start the app built in step 8 and open **Settings → Account → Sign in or create an account**.
2. Create an account. The code should arrive by email, or appear in `journalctl` if email isn't configured. Enter it.
3. Settings now shows "Signed in as …" and "Last synced …".
4. On a second computer, sign in to the same account. Notes and highlights appear there within about two minutes, or straight away with **Sync now**.

## Updating the server later

```sh
# copy the new code (step 3), then on the VPS:
cd ~/study-library-server
cargo build --release
sudo install -m 755 target/release/study-library-server /usr/local/bin/study-library-server
sudo systemctl restart study-library-server
```

The database tables are created and kept automatically, so no migration step is needed.

## Backups

Everything is in one file, `/var/lib/study-library/accounts.db`. A daily safe copy:

```sh
sudo nano /etc/cron.daily/study-library-backup
```

```sh
#!/bin/sh
mkdir -p /var/backups/study-library
sqlite3 /var/lib/study-library/accounts.db ".backup '/var/backups/study-library/accounts-$(date +%F).db'"
find /var/backups/study-library -name 'accounts-*.db' -mtime +14 -delete
```

```sh
sudo chmod +x /etc/cron.daily/study-library-backup
```

Copy `/var/backups/study-library` off the VPS from time to time, for example with `rsync` to your computer.

## Limits on sign-in attempts

The server already slows down password guessing on its own:

- Each client address may make 30 requests per 10 minutes to the sign-in, sign-up, code and password-reset endpoints. After that, the app shows "Too many attempts".
- Each emailed code allows 5 tries.

The client address comes from the `X-Forwarded-For` header that Caddy adds. The server trusts that header only when the connection comes from a loopback or private address (a proxy on the same machine, or the Caddy container); a client connecting directly is counted by its own address.

## Troubleshooting

| Problem | Check |
|---|---|
| `curl https://…/v1/config` times out | Is the DNS pointing to the VPS (`dig +short …`)? Are ports 80 and 443 open (`sudo ufw status`), including in the VPS provider's own firewall panel? |
| Caddy shows a certificate error | `sudo journalctl -u caddy -e`. Port 80 must be reachable for Let's Encrypt. |
| `502 Bad Gateway` | The server isn't running: `sudo systemctl status study-library-server` and `sudo journalctl -u study-library-server -e` |
| The service won't start: "cannot open the database" | `sudo chown -R studylib:studylib /var/lib/study-library` |
| Codes never arrive | `journalctl` shows the SMTP error. Check the host, port (587 vs 465), username/password, and that `SMTP_FROM` is allowed by the provider. Look in spam, and add SPF/DKIM. |
| The app says "Can't reach the account server" | Is `server_url` in `app/account.conf` exactly your https address, and did you rebuild the app after changing it? |
| The Google button doesn't appear | `curl …/v1/config` must show your client ID. Restart the service after editing `server.env`. |
| Google says "access blocked" / "app not verified" | Publish the consent screen (step 9.2), or add your Google account as a test user. |
