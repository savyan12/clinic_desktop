# Physiotherapy Clinic API

Laravel 13 API for the single-branch physiotherapy clinic management system.

## Requirements

- PHP 8.4 with `bcmath`, `gd`, `mbstring`, `pdo_mysql`, and `pdo_sqlite`
- Composer 2
- MySQL 8 for the production-like database
- Node/npm only if the default Laravel asset bundle is used

The included `docker-compose.yml` starts MySQL only when Docker is available.

## Setup

```bash
composer install
cp .env.example .env
php artisan key:generate
php artisan migrate:fresh --seed
php artisan serve
```

The API base URL is `http://localhost:8000/api/v1`.

## Development credentials

> Development only. Change or remove these accounts before deployment.

| Role | Username | Password |
| --- | --- | --- |
| Admin | `admin` | `Admin123!` |
| Receptionist | `reception` | `Reception123!` |
| Receptionist | `reception2` | `Reception123!` |

Login uses `username` and `password`. Authenticated requests use the returned Sanctum bearer token.

## Tests and formatting

```bash
php artisan test --compact
vendor/bin/pint --format agent
```

Tests use SQLite in memory by default. Environment verification should also run `php artisan migrate:fresh --seed` and the full suite against MySQL 8.

For a dedicated MySQL test database with suitable permissions:

```bash
DB_CONNECTION=mysql DB_DATABASE=system_backend_test php artisan test --compact
```

Phase 2 was verified successfully against MySQL 8.4.13 as well as the default SQLite test database.

## Money and idempotency

- Stored money uses `DECIMAL(12,3)` and application calculations use `App\Support\Money` with BCMath strings.
- Money-affecting POST requests require `Idempotency-Key: <uuid>`.
- Financial rows are never deleted through API routes; refunds, reversals, and void states preserve history.

## Report exports

Admin report endpoints support `format=xlsx` through OpenSpout and `format=pdf` through mPDF. PDF output is Arabic RTL and includes the report period and generation time.
