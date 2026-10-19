# Zploy Deployment & Operations Guide

This guide describes how to run and operate Zploy in a production Linux or Windows server environment.

## 1. System Requirements

* **Operating System**: Linux (Ubuntu 22.04 LTS, Debian 12, Rocky Linux) or Windows Server 2022.
* **CPU**: 1 core minimum.
* **RAM**: 512 MB minimum (Zploy typically consumes ~15-25 MB of resident memory).
* **Disk**: Fast SSD recommended for serving static assets.
* **Database**: MySQL 8.0+ or MariaDB 10.5+.

## 2. Production Installation

### 2.1 Build Release Binary
```bash
git clone https://github.com/kiyogo/zploy.git
cd zploy
cargo build --release
```
The optimized executable will be located at `target/release/zploy`.

### 2.2 Systemd Service (Linux)

Create `/etc/systemd/system/zploy.service`:

```ini
[Unit]
Description=Zploy Static Web Hosting Server
After=network.target mysql.service

[Service]
Type=simple
User=www-data
Group=www-data
WorkingDirectory=/var/www/zploy
ExecStart=/var/www/zploy/zploy
Restart=always
RestartSec=5
LimitNOFILE=65536
Environment="DATABASE_URL=mysql://zploy_user:secret@127.0.0.1:3306/zploy"
Environment="ZPLOY_PORT=3000"

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now zploy
sudo systemctl status zploy
```

### 2.3 Reverse Proxy Configuration

#### Caddy (Recommended)
`/etc/caddy/Caddyfile`:
```caddy
example.com {
    reverse_proxy 127.0.0.1:3000 {
        header_up X-Forwarded-Proto {scheme}
        header_up X-Real-IP {remote_host}
    }
}
```

#### Nginx
```nginx
server {
    listen 80;
    server_name example.com;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name example.com;

    ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;

    client_max_body_size 100M;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

## 3. Maintenance & Operations

### 3.1 Resetting Administrator Credentials
```bash
./target/release/reset_admin admin NewPassword123
```

### 3.2 Backup Database & Storage
```bash
# Database backup
mysqldump -u root -p zploy > zploy_backup.sql

# Storage archive backup
tar -czvf storage_backup.tar.gz storage/sites/
```
