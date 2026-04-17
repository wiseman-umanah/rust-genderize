# HNG Stage 1 Task: Data Persistence & API Design Assessment

A RESTful API that integrates with multiple external APIs, applies classification logic, stores data in a database, and provides endpoints to manage that data.

## Features

### External API Integration
- **Genderize API** - Gender prediction with probability and sample size
- **Agify API** - Age prediction with automatic age group classification
- **Nationalize API** - Country prediction with highest probability selection

### Classification Logic
- **Age Groups**: 0-12 (child), 13-19 (teenager), 20-59 (adult), 60+ (senior)
- **Nationality**: Selects country with highest probability from Nationalize API
- **Validation**: Handles invalid API responses with proper error codes

### Database Persistence
- SQLite database with migrations
- UUID v7 for profile IDs
- Duplicate detection by name
- Proper data modeling with relationships

## API Endpoints

### POST /api/profiles
Creates a new profile with external API data integration.

**Request:**
```json
{
  "name": "ella"
}
```

**Success Response (201 Created):**
```json
{
  "status": "success",
  "data": {
    "id": "b3f9c1e2-7d4a-4c91-9c2a-1f0a8e5b6d12",
    "name": "ella",
    "gender": "female",
    "gender_probability": 0.99,
    "sample_size": 1234,
    "age": 46,
    "age_group": "adult",
    "country_id": "DRC",
    "country_probability": 0.85,
    "created_at": "2026-04-01T12:00:00Z"
  }
}
```

**Duplicate Response (200 OK):**
```json
{
  "status": "success",
  "message": "Profile already exists",
  "data": { ...existing profile... }
}
```

### GET /api/profiles/{id}
Retrieves a single profile by ID.

**Success Response (200 OK):**
```json
{
  "status": "success",
  "data": {
    "id": "b3f9c1e2-7d4a-4c91-9c2a-1f0a8e5b6d12",
    "name": "emmanuel",
    "gender": "male",
    "gender_probability": 0.99,
    "sample_size": 1234,
    "age": 25,
    "age_group": "adult",
    "country_id": "NG",
    "country_probability": 0.85,
    "created_at": "2026-04-01T12:00:00Z"
  }
}
```

### GET /api/profiles
Retrieves all profiles with optional filtering.

**Query Parameters (Optional):**
- `gender` - Filter by gender (case-insensitive)
- `country_id` - Filter by country ID (case-insensitive)
- `age_group` - Filter by age group (case-insensitive)

**Example:** `/api/profiles?gender=male&country_id=NG`

**Success Response (200 OK):**
```json
{
  "status": "success",
  "count": 2,
  "data": [
    {
      "id": "id-1",
      "name": "emmanuel",
      "gender": "male",
      "age": 25,
      "age_group": "adult",
      "country_id": "NG"
    },
    {
      "id": "id-2",
      "name": "sarah",
      "gender": "female",
      "age": 28,
      "age_group": "adult",
      "country_id": "US"
    }
  ]
}
```

### DELETE /api/profiles/{id}
Deletes a profile by ID.

**Success Response (204 No Content)**

## Error Handling

All errors follow this structure:
```json
{
  "status": "error",
  "message": "<error message>"
}
```

**Error Codes:**
- `400 Bad Request` - Missing or empty name
- `404 Not Found` - Profile not found
- `422 Unprocessable Entity` - Invalid type
- `502 Bad Gateway` - External API returned invalid response
- `500 Internal Server Error` - Database or server error

## Technology Stack

- **Language**: Rust
- **Web Framework**: Axum
- **Database**: SQLite with sqlx
- **HTTP Client**: reqwest
- **Serialization**: serde
- **Async Runtime**: tokio
- **CORS**: tower-http

## Getting Started

### Prerequisites
- Rust 1.70+
- SQLite3

### Installation
```bash
git clone <repository-url>
cd stage_zero
cargo build
```

### Running the Server
```bash
cargo run
```

The server will start on `http://localhost:3000`

## Testing

### Create a Profile
```bash
curl -X POST http://localhost:3000/api/profiles \
  -H "Content-Type: application/json" \
  -d '{"name": "ella"}'
```

### Get All Profiles
```bash
curl http://localhost:3000/api/profiles
```

### Get Single Profile
```bash
curl http://localhost:3000/api/profiles/{id}
```

### Delete Profile
```bash
curl -X DELETE http://localhost:3000/api/profiles/{id}
```

## Database Schema

The application uses SQLite with the following schema:

```sql
CREATE TABLE profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    gender TEXT NOT NULL,
    gender_probability REAL NOT NULL,
    sample_size INTEGER NOT NULL,
    age INTEGER NOT NULL,
    age_group TEXT NOT NULL,
    country_id TEXT NOT NULL,
    country_probability REAL NOT NULL,
    created_at TEXT NOT NULL
);
```

## Edge Cases Handled

- Genderize returns `gender: null` or `count: 0` -> Returns 502 error
- Agify returns `age: null` -> Returns 502 error  
- Nationalize returns no country data -> Returns 502 error
- Duplicate profile names -> Returns existing profile
- Empty or missing names -> Returns 400 error

## CORS

The API supports Cross-Origin Resource Sharing with `Access-Control-Allow-Origin: *` header to enable frontend integration.


