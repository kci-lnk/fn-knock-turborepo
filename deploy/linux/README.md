# fn-knock Linux

Linux releases target systemd and include `amd64`, `arm64`, and `armv7` bundles.
Build them from the repository root with:

```bash
npm run fn-knock:linux:prepare
```

The installer is published as `install.sh`. It first detects whether fn-knock
is already installed and lets the user install/update, open the management
menu, check status, or uninstall. Before activation it checks all five runtime
ports. If a port is occupied, the installer opens the port configuration menu
so a replacement can be selected before continuing.

The management panel defaults to `0.0.0.0:7991`. For public Internet use,
place it behind an HTTPS reverse proxy with access controls instead of exposing
7991 directly:

```nginx
server {
    listen 443 ssl http2;
    server_name knock.example.com;
    ssl_certificate /etc/nginx/ssl/knock.example.com.fullchain.pem;
    ssl_certificate_key /etc/nginx/ssl/knock.example.com.key;

    location / {
        proxy_pass http://127.0.0.1:7991;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 3600s;
    }
}
```

Run `sudo knock nginx` to print the same template after installation. Linux
runtime mode never manages host firewall rules and does not invoke iptables.

Run `sudo knock config` to change ports after installation. The command shows
the current mapping, offers a one-step panel-port change for the common case,
and exposes the other listeners only through an advanced option. It will not
save a configuration that uses a duplicate or occupied port.
