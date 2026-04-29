# Insighta Labs+

Insighta Labs+ Backend is the Rust/Axum API server for the Stage 3 platform. It expands the Stage 2 API with versioned routes, GitHub OAuth foundation, role-aware API access, refresh-token rotation, CSV export, request logging, and rate limiting.

## Architecture

```text
stage_zero/
  Cargo.toml
  src/
  migrations/
  .github/workflows/ci.yml
```

This repository is the backend repo. The CLI and web portal live in separate sibling repositories:

```text
../insighta-cli
../insighta-web
```

## Backend

Run from this repository root:

```bash
cargo run
```

Default server URL:

```text
http://localhost:3000
```

Environment variables:

```text
DATABASE_URL=sqlite://./data.db
HOST=0.0.0.0
PORT=3000
GITHUB_CLIENT_ID=
GITHUB_CLIENT_SECRET=
GITHUB_REDIRECT_URL=http://localhost:3000/auth/github/callback
JWT_SECRET=change-me
BACKEND_BASE_URL=http://localhost:3000
WEB_BASE_URL=http://localhost:5173
```

### Auth Endpoints

```http
GET  /auth/github
GET  /auth/github/callback
POST /auth/refresh
POST /auth/logout
GET  /auth/csrf
```

GitHub OAuth creates or updates users. New users default to the `analyst` role. Access tokens expire after 3 minutes, refresh tokens after 5 minutes, and refresh tokens are rotated after use.

### Versioned API

All `/api/*` requests require:

```http
X-API-Version: 1
```

Profile routes:

```http
GET    /api/classify
GET    /api/profiles
POST   /api/profiles
GET    /api/profiles/{id}
DELETE /api/profiles/{id}
GET    /api/profiles/search
GET    /api/profiles/export?format=csv
```

Role rules:

```text
admin   = read, search, export, create, delete
analyst = read, search, export
```

## Natural Language Search

The backend keeps the Stage 2 rule-based natural language parser. It recognizes gender, age groups, age ranges, country names, country codes, and demonyms from the live profile country mapping.

Example:

```bash
curl "http://localhost:3000/api/profiles/search?q=young%20males%20from%20nigeria" \
  -H "X-API-Version: 1" \
  -H "Authorization: Bearer <access_token>"
```

## Verification

```bash
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```
