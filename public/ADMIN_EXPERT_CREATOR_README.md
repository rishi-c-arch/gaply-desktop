# Admin Expert Creator

A standalone admin page for creating expert profiles on the Gaply platform.

## Features

### 🎯 **Easy Profile Creation**
- **Simple Form Interface** - Clean, intuitive form for expert data entry
- **Image Upload** - Direct image upload with preview
- **Domain Selection** - Choose from predefined research domains
- **Real-time Validation** - Form validation with helpful error messages
- **Instant Preview** - See image preview before submission

### 📊 **Comprehensive Data Entry**
- **Basic Information** - Name, email, bio
- **Experience Stats** - Years of experience, publications, patents
- **Research Domains** - Multiple domain selection
- **Availability Status** - Set expert availability
- **Profile Image** - Professional photo upload

### 🔧 **Technical Features**
- **Standalone Page** - Independent HTML page, not part of main website
- **Direct Database Integration** - Creates profiles directly in database
- **Image Storage** - Local file storage with unique naming
- **Admin Authentication** - Simple token-based admin access
- **Responsive Design** - Works on all devices

## Access

### URL
```
http://localhost:3000/admin-expert-creator.html
```

### Admin Authentication
The page uses a simple token-based authentication system:

**Demo Token**: `demo_token`
**Admin Token**: `admin_token`

Add the token to localStorage:
```javascript
localStorage.setItem('admin_token', 'demo_token');
```

## Usage

### 1. **Basic Information**
- Enter expert's full name
- Provide email address
- Write a compelling bio describing their expertise

### 2. **Profile Image**
- Click "Upload Profile Image" button
- Select an image file (JPG, PNG, etc.)
- Preview will appear automatically
- Images are stored in `/uploads/expert-images/`

### 3. **Experience & Statistics**
- **Experience**: Years of professional experience
- **Publications**: Number of published papers
- **Patents**: Number of patents held

### 4. **Research Domains**
- Select from dropdown menu
- Click "Add Domain" to add to profile
- Multiple domains can be selected
- Remove domains by clicking the × button

### 5. **Availability**
- Check/uncheck "Expert is available for new projects"
- Controls whether expert appears in search results

### 6. **Submit**
- Click "Create Expert Profile" button
- Profile is created in database
- Success/error message is displayed
- Form resets automatically on success

## API Endpoints

### Create Profile with Image
```
POST /api/admin/expert/profile-with-image
Content-Type: multipart/form-data
Authorization: Bearer admin_token

Form Data:
- name: string (required)
- email: string (required)
- bio: string (required)
- experience: number
- publications: number
- patents: number
- domains: JSON array
- is_available: boolean
- image: file (optional)
```

### Get Profile Image
```
GET /api/admin/expert/profile/:id/image
```

### Update Profile with Image
```
PUT /api/admin/expert/profile/:id/with-image
Content-Type: multipart/form-data
Authorization: Bearer admin_token
```

## Database Schema

The profiles are stored in the `expert_profiles` table:

```sql
CREATE TABLE expert_profiles (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL UNIQUE,
    domains TEXT[] NOT NULL DEFAULT '{}',
    experience INTEGER NOT NULL DEFAULT 0,
    publications INTEGER NOT NULL DEFAULT 0,
    patents INTEGER NOT NULL DEFAULT 0,
    bio TEXT,
    image_url VARCHAR(500),
    is_available BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
```

## File Storage

### Image Storage Location
```
uploads/expert-images/
├── 1640995200_Dr_John_Smith.jpg
├── 1640995300_Dr_Jane_Doe.png
└── ...
```

### File Naming Convention
```
{timestamp}_{name}_{extension}
```

### Image URL Format
```
/uploads/expert-images/{filename}
```

## Available Research Domains

1. **Data Analysis & Statistics**
2. **Machine Learning / AI Engineer**
3. **Research & Writing**
4. **Health & Medicine**
5. **Business & Economics**
6. **Technology & Engineering**
7. **Compliance & Ethics**
8. **Legal & Policy**
9. **Humanities & Social Sciences**
10. **Science & Technology**
11. **Data & Analytics**
12. **Gender & Society**
13. **Emerging Technologies**
14. **Interdisciplinary Research**

## Security Features

### Admin Authentication
- Token-based authentication
- Admin-only access to creation endpoints
- Simple token validation

### File Upload Security
- File type validation (images only)
- File size limits (32MB max)
- Unique filename generation
- Secure file storage

### Input Validation
- Required field validation
- Email format validation
- Numeric field validation
- SQL injection protection

## Error Handling

### Common Errors
- **Missing Required Fields**: Name, email, or bio not provided
- **Invalid Email Format**: Email doesn't match valid format
- **File Upload Error**: Image upload failed
- **Database Error**: Profile creation failed
- **Authentication Error**: Invalid or missing admin token

### Error Messages
All errors are displayed in a user-friendly format with:
- Clear error description
- Suggested actions
- Visual error styling (red background)

## Integration with Main Website

### Search Integration
Created profiles automatically appear in:
- Expert search results
- Domain-based filtering
- Featured experts section
- Public search API

### Profile Display
Profiles are displayed on the main website with:
- Professional image
- Name and bio
- Experience statistics
- Research domains
- Contact functionality

## Development

### Local Development
1. Start the backend server
2. Open `admin-expert-creator.html` in browser
3. Set admin token in localStorage
4. Create test profiles

### Production Deployment
1. Deploy backend with image upload support
2. Configure file storage (consider cloud storage)
3. Set up proper admin authentication
4. Deploy HTML page to web server

## Future Enhancements

- **Bulk Upload**: CSV/Excel import for multiple profiles
- **Image Optimization**: Automatic image resizing and compression
- **Cloud Storage**: Integration with AWS S3 or similar
- **Advanced Authentication**: JWT-based admin system
- **Profile Templates**: Predefined templates for common expert types
- **Analytics**: Track profile creation and usage statistics
- **Email Notifications**: Notify experts when profiles are created
- **Profile Validation**: Automated profile quality checks
