# fn-knock Linux

Linux releases target systemd and include `amd64`, `arm64`, and `armv7` bundles.
Build them from the repository root with:

```bash
npm run fn-knock:linux:prepare
```

The installer is published as `install.sh`. It keeps the control plane on
`127.0.0.1:7991`; expose it through an HTTPS reverse proxy instead of opening
7991 to the Internet:

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
