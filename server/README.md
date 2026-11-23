# Study Library account server

Optional accounts for Study Library. Signed-in devices keep these in step:

- notes, with their tags
- highlights
- bookmarks
- favorites
- playlists
- the list of downloaded publications

The app works without an account.

## Run

```sh
cargo build --release
BIND=127.0.0.1:5764 DB_PATH=/var/lib/study-library/accounts.db ./target/release/study-library-server
```

Or with Docker, which includes Caddy with automatic HTTPS certificates:

```sh
cp server.env.example server.env    # fill in SMTP_* / GOOGLE_*
cp .env.example .env                # set DOMAIN
docker compose up -d --build
```

Serve it over HTTPS. The usual way is a reverse proxy in front of `BIND`, for example Caddy:

```
accounts.example.org {
    reverse_proxy 127.0.0.1:5764
}
```

Then put that address in `app/account.conf` (`server_url = "https://accounts.example.org"`) and rebuild the app. The address is built into the program. Full step-by-step instructions for a VPS are in [DEPLOY.md](DEPLOY.md).

## Settings (environment variables)

| Variable | Meaning |
|---|---|
| `BIND` | Listen address (default `127.0.0.1:5764`) |
| `DB_PATH` | SQLite database file (default `./accounts.db`) |
| `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASS`, `SMTP_FROM` | Mail server for the 6-digit codes. Port 587 or 25 uses STARTTLS; 465 (the default) uses TLS from the start. When `SMTP_HOST` is unset the codes are printed to the server log, which is useful for testing. |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | Turn on "Continue with Google" |

## Setting up Google sign-in

1. Go to <https://console.cloud.google.com/> and create or choose a project.
2. Open **APIs & Services → OAuth consent screen**. Fill in the app name and support email, and add the scopes `openid`, `email` and `profile`. Publish the app; while it is in testing, only listed test users can sign in.
3. Open **APIs & Services → Credentials → Create credentials → OAuth client ID**. Choose the type **Desktop app**.
4. Put the client ID and secret in `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` on the server.

The secret stays on the server and the app never contains it. The app opens Google in the browser and receives the answer on a one-time `http://127.0.0.1:<port>` address. It passes the code to `/v1/auth/google`, and the server exchanges it with Google.

Apple sign-in is not built yet. It needs a paid Apple developer account, a Services ID, and an HTTPS return address on this server.

## API (JSON)

| Method and path | Body | Answer |
|---|---|---|
| `GET /v1/config` | | `{google_client_id}` |
| `POST /v1/auth/signup` | `{email, password, device}` | Emails a code |
| `POST /v1/auth/resend` | `{email}` | |
| `POST /v1/auth/verify` | `{email, code, device}` | `{token, user}` |
| `POST /v1/auth/login` | `{email, password, device}` | `{token, user}`, or 403 `unverified` (a new code is sent) |
| `POST /v1/auth/forgot` | `{email}` | Emails a reset code; the answer is the same whether or not the account exists |
| `POST /v1/auth/reset` | `{email, code, password, device}` | `{token, user}`; signs out every other device |
| `POST /v1/auth/google` | `{code, code_verifier, redirect_uri, device}` | `{token, user}` |
| `POST /v1/auth/logout` | | Needs `Authorization: Bearer <token>` |
| `GET /v1/me` | | Needs `Authorization: Bearer <token>` |
| `DELETE /v1/account` | | Deletes the user, their sessions and synced data. Needs `Authorization: Bearer <token>` |
| `POST /v1/sync` | `{since, device, changes:[{kind, key, data, deleted}]}` | `{seq, changes}`. Needs `Authorization: Bearer <token>` |

Errors look like `{error: "<message to show>", code: "<machine code>"}`.

How sync works:

- The server gives every stored change the user's next sequence number. If two devices change the same item, the last one to sync wins.
- A device sends what changed since its last sync and receives every item changed after its last `seq`.
- Passwords are stored as Argon2 hashes. Session tokens and codes are stored only as SHA-256 hashes.
- A code expires after 15 minutes or 5 wrong tries. A new one can be requested once a minute.
- Sign-in, sign-up, code and password-reset requests are limited to 30 per 10 minutes per client address. Behind a proxy on the same machine (or the Caddy container) the address comes from `X-Forwarded-For`; otherwise it is the connection's own address.

## Not done yet

- Apple sign-in.
- A list of signed-in devices.
