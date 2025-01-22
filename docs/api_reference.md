# Waifu API Reference

## Authentication
All endpoints require authentication via a Bearer token in the Authorization header.

```sh
Authorization: Bearer <your_api_key>
```

There are two types of API keys:

1. **Admin Key**: Has full access to all endpoints and no rate/batch limits.
2. **User Key**: Has configurable rate limits and batch limits.


## Rate Limiting
- Each API key can have a requests-per-second limit.
- Rate limits are applied per key, not per endpoint.
- Admin key has no rate limits.
- Exceeding rate limits returns 429 Too Many Requests.


## Endpoints

### Health Check
```sh
GET /health
```

Returns server status and timestamp. Does not require authentication.

**Response:**
```json
{
  "status": "ok",
  "timestamp": "2025-01-01T00:00:00Z"
}
```

### Random Image(s)
Supports both GET and POST methods for different use cases.

#### GET /random
Returns a single random image matching the specified filters.

**Query Parameters:**
- `tags` - Comma-separated list of tags (e.g., `?tags=cat,cute`)
- `width` - Exact width in pixels
- `width_min`, `width_max` - Width range in pixels
- `height` - Exact height in pixels
- `height_min`, `height_max` - Height range in pixels
- `size` - Exact file size in bytes
- `size_min`, `size_max` - File size range in bytes

**Example:**
```bash
# Get a random image tagged with both 'cat' and 'cute', between 800 and 1920 pixels wide
curl "http://localhost:8000/random?tags=cat,cute&width_min=800&width_max=1920" \
  -H "Authorization: Bearer your_api_key"
```

**Response:**
```json
{
  "url": "http://localhost:8000/images/image1.jpg",
  "filename": "image1.jpg",
  "format": "JPEG",
  "width": 1024,
  "height": 768,
  "size_bytes": 123456,
  "hash": "abc123...",
  "tags": ["cat", "cute"],
  "created_at": "2024-01-22T06:24:29Z",
  "modified_at": "2024-01-22T06:24:29Z"
}
```

#### POST /random
Returns multiple random images matching the specified filters. The number of images is controlled by the `count` parameter in the request body.

**Request Body:**
```json
{
  "count": 3,                   // Required: Number of images to return
  "tags": ["cat", "cute"],      // Optional: Array of tags to match
  "width": 1920,                // Optional: Exact width in pixels
  "width_min": 800,             // Optional: Minimum width in pixels
  "width_max": 1920,            // Optional: Maximum width in pixels
  "height": 1080,               // Optional: Exact height in pixels
  "height_min": 600,            // Optional: Minimum height in pixels
  "height_max": 1080,           // Optional: Maximum height in pixels
  "size": 1048576,              // Optional: Exact file size in bytes
  "size_min": 524288,           // Optional: Minimum file size in bytes
  "size_max": 2097152           // Optional: Maximum file size in bytes
}
```

**Example:**
```bash
# Get 3 random images with filters
curl -X POST http://localhost:8000/random \
  -H "Authorization: Bearer your_api_key" \
  -H "Content-Type: application/json" \
  -d '{
    "count": 3,
    "tags": ["cat", "cute"],
    "width_min": 800,
    "width_max": 1920
  }'
```

**Response:**
```json
{
  "images": [
    {
      "url": "http://localhost:8000/images/image1.jpg",
      "filename": "image1.jpg",
      "format": "JPEG",
      "width": 1024,
      "height": 768,
      "size_bytes": 123456,
      "hash": "abc123...",
      "tags": ["cat", "cute"],
      "created_at": "2024-01-22T06:24:29Z",
      "modified_at": "2024-01-22T06:24:29Z"
    },
    // ... more images ...
  ],
  "total": 3,
  "successful": 3,
  "failed": 0,
  "errors": []
}
```

**Notes:**
1. The GET method is a convenience wrapper around POST, limited to returning a single image
2. The POST method's `count` parameter must not exceed the API key's `max_batch_size`
3. All filter parameters are optional
4. When using both min/max filters, min must be less than or equal to max
5. Tags are matched exactly and all specified tags must be present
6. The admin key has no batch size limits
7. If fewer images are found than requested, the response will include all found images and indicate the difference in the counts
8. Filter parameters can be combined to narrow down results
9. Empty filter parameters are ignored (not applied to the query)

### Batch Random Images
```sh
POST /random
```

Returns multiple random images matching the specified (optional) filters.

**Query Parameters:**
Same as GET /random.

**Request Body:**
```js
{
  "count": 3  // Number of images to return (must not exceed max_batch_size)
}
```

**Example**:

```sh
curl -X POST "http://localhost:8000/random" \
  -H "Authorization: Bearer your_api_key" \
  -H "Content-Type: application/json" \
  -d '{"count": 3}'
```

**Response:**
```json
{
  "images": [
    {
      "url": "http://localhost:8000/images/image1.jpg",
      "filename": "image1.jpg",
      "format": "JPEG",
      "width": 1024,
      "height": 768,
      "size_bytes": 123456,
      "hash": "abc123...",
      "tags": ["cat", "cute"],
      "created_at": "2024-01-22T06:24:29Z",
      "modified_at": "2024-01-22T06:24:29Z"
    },
    // ... more images ...
  ],
  "total": 3,
  "successful": 3,
  "failed": 0,
  "errors": []
}
```

### Add Single Image
```sh
POST /images
```

Adds a single image to the database.

**Request Body:**
```js
{
  "path": "/path/to/image.jpg",
  "type": "local",  // "local" or "url"
  "tags": ["tag1", "tag2"]
}
```

**Example**:
```sh
curl -X POST http://localhost:8000/images \
  -H "Authorization: Bearer your_api_key" \
  -H "Content-Type: application/json" \
  -d '{
    "path": "/home/user/images/cat.jpg",
    "type": "local",
    "tags": ["cat", "cute"]
  }'
```

**Response:**
```js
{
  "message": "Image added successfully",
  "hash": "abc123...",
  "tags": ["cat", "cute"]
}
```

### Batch Add Images
```sh
POST /images
```

Adds multiple images in a single request.

**Request Body:**
```js
{
  "images": [
    {
      "path": "/path/to/image1.jpg",
      "type": "local",
      "tags": ["tag1", "tag2"]
    },
    {
      "path": "https://example.com/image2.jpg",
      "type": "url",
      "tags": ["tag3", "tag4"]
    }
  ]
}
```

**Example**:
```sh
curl -X POST http://localhost:8000/images \
  -H "Authorization: Bearer your_api_key" \
  -H "Content-Type: application/json" \
  -d '{
    "images": [
      {
        "path": "/home/user/images/cat1.jpg",
        "type": "local",
        "tags": ["cat", "sleeping"]
      },
      {
        "path": "/home/user/images/cat2.jpg",
        "type": "local",
        "tags": ["cat", "playing"]
      }
    ]
  }'
```

**Response:**
```js
{
  "message": "Batch processing completed",
  "total": 2,
  "successful": 2,
  "failed": 0,
  "results": [
    {
      "hash": "abc123...",
      "tags": ["cat", "sleeping"]
    },
    {
      "hash": "def456...",
      "tags": ["cat", "playing"]
    }
  ],
  "errors": []
}
```


### Delete Image
```sh
DELETE /images/{filename}
```

Deletes an image by its unique filename. Requires admin key.

**Example:**
```sh
curl -X DELETE http://localhost:8000/images/image1.jpg \
  -H "Authorization: Bearer your_admin_key"
```

Returns 200 OK if successful.

### Get All Tags
```sh
GET /tags
```

Returns a list of all tags in the database, deduplicated.

**Example:**
```sh
curl http://localhost:8000/tags \
  -H "Authorization: Bearer your_api_key"
```

**Response:**
```js
{
    "tags": [
        {
            "count": 1,
            "name": "cat"
        },
        {
            "count": 2,
            "name": "cute"
        }
    ],
    "total_tags": 2
}
```

### API Key Management (Admin Only)

#### Generate API Key
```sh
POST /api-keys
```

Creates a new API key.

**Request Body:**
```js
{
  "username": "user1",
  "requests_per_second": 10,  // optional, null for unlimited
  "max_batch_size": 5        // optional, null for unlimited
}
```

**Example:**
```sh
curl -X POST http://localhost:8000/api-keys \
  -H "Authorization: Bearer your_admin_key" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "user1",
    "requests_per_second": 10,
    "max_batch_size": 5
  }'
```


#### List API Keys
```sh
GET /api-keys
```

Lists all API keys.

**Example:**
```sh
curl http://localhost:8000/api-keys \
  -H "Authorization: Bearer your_admin_key"
```


**Response:**
```js
[
  {
    "key": "ef815a08-ade5-415a-b3b6-80776e91068b",
    "username": "non_batch_user",
    "created_at": "2025-01-22T06:29:52.474231728Z",
    "last_used_at": "2025-01-22T06:30:15.403983154Z",
    "is_active": true,
    "requests_per_second": 10,
    "max_batch_size": 1
  },
  {
    "key": "3218fb1b-8817-4f74-b73d-4904c14dc1fb",
    "username": "batch_user",
    "created_at": "2025-01-22T06:21:25.527467243Z",
    "last_used_at": "2025-01-22T06:30:27.973797636Z",
    "is_active": true,
    "requests_per_second": 10,
    "max_batch_size": 5
  }
]
```

#### Remove API Key
```sh
DELETE /api-keys
```

Deletes an API key.

**Example:**
```sh
curl -s -X DELETE http://localhost:8000/api-keys \
  -H "Authorization: Bearer your_admin_key" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "non_batch_user"
  }'
```

**Response:**

```js
{
  "message": "API key for user 'non_batch_user' was successfully removed"
}
```

If the username is not found, returns 404 Not Found.
```js
{
  "code": 404,
  "message": "The username 'non_batch_user' was not found",
  "request_id": "435ae425-669d-4ff7-abbe-b1a8b7cb49c2"
}

```

#### Update API Key Status
```sh
PATCH /api-keys/{username}/status
```

Updates the status of a user's API key, from active to inactive.

**Example:**
```sh
curl -X PATCH http://localhost:8000/api-keys/batch_user/status \
  -H "Authorization: Bearer your_admin_key" \
  -H "Content-Type: application/json" \
  -d '{
    "is_active": false
  }'
```

This will set the API key to inactive. You can also set it to active by passing `true` in the request body.

**Response:**
```js
{
  "is_active": false,
  "message": "API key status updated successfully",
  "username": "batch_user"
}
```

If the username is not found, returns 404 Not Found.

### Upload Image (Multipart Form)
```sh
POST /upload
```

Uploads a single image with tags using multipart form data.

**Form Fields:**
- `file` - The image file to upload
- `tags` - JSON array of tags as a string (e.g., `'["tag1", "tag2"]'`)

**Example:**
```bash
curl -X POST http://localhost:8000/upload \
  -H "Authorization: Bearer your_api_key" \
  -F "file=@/path/to/image.jpg" \
  -F 'tags=["cat", "cute"]'
```

**Response:**
```json
{
  "message": "Image uploaded successfully",
  "hash": "abc123...",
  "tags": ["cat", "cute"]
}
```

**Notes:**
1. The file must be a valid image (JPEG, PNG, GIF, WebP, or BMP)
2. Maximum file size is 10MB
3. At least one tag is required
4. Tags must be provided as a valid JSON array string
5. The `Content-Type` header is automatically set by the multipart form data
