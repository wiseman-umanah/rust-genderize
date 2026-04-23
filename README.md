# HNG Stage 2 Task: Intelligence Query Engine Assessment

A RESTful API with advanced filtering, sorting, pagination, and natural language search capabilities for demographic intelligence data.

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

### Advanced Query Engine
- **Multi-parameter Filtering**: Combine gender, age, country, and probability filters
- **Sorting**: Sort by age, created_at, or gender_probability (asc/desc)
- **Pagination**: Page-based navigation with configurable limits
- **Natural Language Search**: Query data using plain English phrases

## Natural Language Search

### Parsing Approach

The natural language parser uses rule-based pattern matching to convert plain English queries into structured database filters. The parser follows these principles:

#### **Keyword Recognition**
- **Gender Keywords**: "male", "males", "female", "females"
- **Age Group Keywords**: "child", "children", "teenager", "teenagers", "teen", "teens", "adult", "adults", "senior", "seniors", "elderly"
- **Special Age Keyword**: "young" (maps to ages 16-24)
- **Country Keywords**: Full country names and demonyms (e.g., "nigeria", "nigerian", "united states", "american")

#### **Age Range Parsing**
- **Above/Over**: "above 30", "over 25", "older than 40" → `min_age`
- **Below/Under**: "below 20", "under 18", "younger than 35" → `max_age`
- **Range**: "20 to 30", "25-35", "18 and 25" → `min_age` and `max_age`

#### **Query Examples and Mappings**

| Natural Language Query | Generated Filters |
|----------------------|------------------|
| "young males" | `gender=male` + `min_age=16` + `max_age=24` |
| "females above 30" | `gender=female` + `min_age=30` |
| "people from angola" | `country_id=AO` |
| "adult males from kenya" | `gender=male` + `age_group=adult` + `country_id=KE` |
| "male and female teenagers above 17" | `age_group=teenager` + `min_age=17` |
| "nigerian females between 25 and 35" | `gender=female` + `country_id=NG` + `min_age=25` + `max_age=35` |

#### **Parsing Logic**
1. **Lowercase Conversion**: All queries are converted to lowercase for consistent matching
2. **Token-based Matching**: Uses string contains checks for keyword recognition
3. **Regex Patterns**: Regular expressions for age range extraction
4. **Priority System**: More specific patterns override general ones
5. **Validation**: Queries must contain at least one recognizable filter

### Supported Countries

The parser recognizes 80+ countries including:
- **Major Countries**: Nigeria, United States, United Kingdom, Canada, Australia, Germany, France, Italy, Spain, Brazil, India, China, Japan, South Korea, Russia, Mexico
- **African Countries**: Kenya, Ghana, Egypt, Cameroon, Benin, Angola, Tanzania, Uganda, Rwanda, Ethiopia, Somalia, Libya, Tunisia, Algeria, Morocco, South Africa
- **Asian Countries**: Turkey, Saudi Arabia, Iran, Iraq, Syria, Jordan, Lebanon, Israel, Pakistan, Bangladesh, Sri Lanka, Myanmar, Thailand, Vietnam, Philippines, Indonesia, Malaysia, Singapore
- **Others**: New Zealand, Fiji, Papua New Guinea, and Pacific island nations

### Limitations

#### **Current Limitations**
1. **No Compound Logic**: Cannot handle "AND"/"OR" operators between different filter types
2. **No Negation**: Cannot exclude criteria (e.g., "not male", "without nigeria")
3. **Limited Age Precision**: "young" is fixed to 16-24 range, not context-dependent
4. **No Probability Filters**: Cannot search by confidence levels in natural language
5. **Single Country Recognition**: Only recognizes the first country mentioned in multi-country queries
6. **No Synonym Support**: Limited to predefined keyword lists
7. **Case Sensitivity Issues**: Some edge cases with mixed-case queries
8. **No Temporal Queries**: Cannot handle time-based searches like "recent profiles"

#### **Edge Cases Not Handled**
- **Ambiguous Queries**: "people" without any specific criteria
- **Conflicting Criteria**: "young seniors" (contradictory age ranges)
- **Complex Sentences**: Natural language beyond simple keyword combinations
- **Misspellings**: No fuzzy matching for typos
- **Multiple Age Groups**: "children and adults" (only processes one)
- **Comparative Language**: "older than teenagers but younger than adults"

#### **Future Improvements**
- **Boolean Logic**: Support for AND/OR/NOT operators
- **Fuzzy Matching**: Handle typos and variations
- **Context-Aware Parsing**: Age ranges based on context
- **Probability Language**: "high confidence", "uncertain predictions"
- **Multi-Country Support**: Handle multiple countries in one query

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
    "country_id": "CM",
    "country_name": "Cameroon",
    "country_probability": 0.85,
    "created_at": "2026-04-01T12:00:00Z"
  }
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
    "country_name": "Nigeria",
    "country_probability": 0.85,
    "created_at": "2026-04-01T12:00:00Z"
  }
}
```

### GET /api/profiles
Retrieves profiles with advanced filtering, sorting, and pagination.

**Query Parameters:**
- **Filters**: `gender`, `age_group`, `country_id`, `min_age`, `max_age`, `min_gender_probability`, `min_country_probability`
- **Sorting**: `sort_by` (age|created_at|gender_probability), `order` (asc|desc)
- **Pagination**: `page` (default: 1), `limit` (default: 10, max: 50)

**Example:** `/api/profiles?gender=male&country_id=NG&min_age=25&sort_by=age&order=desc&page=1&limit=10`

**Success Response (200 OK):**
```json
{
  "status": "success",
  "page": 1,
  "limit": 10,
  "total": 2026,
  "data": [
    {
      "id": "b3f9c1e2-7d4a-4c91-9c2a-1f0a8e5b6d12",
      "name": "emmanuel",
      "gender": "male",
      "gender_probability": 0.99,
      "age": 34,
      "age_group": "adult",
      "country_id": "NG",
      "country_name": "Nigeria",
      "created_at": "2026-04-01T12:00:00Z"
    }
  ]
}
```

### GET /api/profiles/search
Natural language search with pagination.

**Query Parameters:**
- `q` - Natural language query (required)
- `page` - Page number (default: 1)
- `limit` - Results per page (default: 10, max: 50)

**Examples:**
- `/api/profiles/search?q=young%20males%20from%20nigeria`
- `/api/profiles/search?q=females%20above%2030`
- `/api/profiles/search?q=adult%20males%20from%20kenya`

**Success Response (200 OK):**
```json
{
  "status": "success",
  "page": 1,
  "limit": 10,
  "total": 156,
  "data": [
    {
      "id": "b3f9c1e2-7d4a-4c91-9c2a-1f0a8e5b6d12",
      "name": "emmanuel",
      "gender": "male",
      "gender_probability": 0.99,
      "age": 22,
      "age_group": "adult",
      "country_id": "NG",
      "country_name": "Nigeria",
      "created_at": "2026-04-01T12:00:00Z"
    }
  ]
}
```

**Error Response (400 Bad Request):**
```json
{
  "status": "error",
  "message": "Unable to interpret query"
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


