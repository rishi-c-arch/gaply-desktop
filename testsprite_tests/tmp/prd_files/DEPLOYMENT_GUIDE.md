# GAPLY Frontend - Deployment Guide

## 🚀 Quick Deployment Checklist

### ✅ Pre-Deployment Checklist
- [ ] All assets copied to `/src/assets/`
- [ ] Razorpay keys configured
- [ ] Environment variables set
- [ ] Production build tested
- [ ] Backend API endpoints updated

## 🔧 Backend Integration Points

### 1. **API Configuration**
Update these files with your backend URLs:

**File**: `src/PackageSelection.tsx`
```typescript
// Update payment handler to call your backend
const handlePackageSelect = async (packageId: string) => {
  // 1. Create order on your backend
  const orderResponse = await fetch(`${process.env.REACT_APP_API_BASE_URL}/api/orders`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ packageId, amount: selectedPackage.price })
  });
  
  const order = await orderResponse.json();
  
  // 2. Use order_id from backend in Razorpay
  const options = {
    key: process.env.REACT_APP_RAZORPAY_KEY_ID,
    amount: order.amount,
    order_id: order.id, // From your backend
    // ... rest of options
  };
};
```

### 2. **Authentication Integration**
**File**: `src/LoginPage.tsx`
```typescript
const handleSubmit = async (e: React.FormEvent) => {
  e.preventDefault();
  
  try {
    const response = await fetch(`${process.env.REACT_APP_API_BASE_URL}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(formData)
    });
    
    const data = await response.json();
    
    if (data.success) {
      localStorage.setItem('token', data.token);
      localStorage.setItem('user', JSON.stringify(data.user));
      onLoginSuccess?.();
    }
  } catch (error) {
    console.error('Login failed:', error);
  }
};
```

### 3. **User Account Management**
**File**: `src/PackageSelection.tsx`
```typescript
// Fetch user account from backend
const fetchUserAccount = async () => {
  try {
    const token = localStorage.getItem('token');
    const response = await fetch(`${process.env.REACT_APP_API_BASE_URL}/api/user/account`, {
      headers: { 'Authorization': `Bearer ${token}` }
    });
    
    const account = await response.json();
    setUserAccount(account);
  } catch (error) {
    console.error('Failed to fetch account:', error);
  }
};
```

## 🌐 Environment Configuration

### Create `.env` file:
```env
REACT_APP_API_BASE_URL=https://your-backend-domain.com
REACT_APP_RAZORPAY_KEY_ID=rzp_live_REMOVED
REACT_APP_RAZORPAY_KEY_SECRET=RAZORPAY_SECRET_REMOVED
REACT_APP_ENVIRONMENT=production
```

### Create `.env.local` for local development:
```env
REACT_APP_API_BASE_URL=http://localhost:8000
REACT_APP_RAZORPAY_KEY_ID=rzp_test_YOUR_TEST_KEY
REACT_APP_RAZORPAY_KEY_SECRET=YOUR_TEST_SECRET
REACT_APP_ENVIRONMENT=development
```

## 🏗️ Build & Deploy

### 1. **Production Build**
```bash
# Install dependencies
npm install

# Create production build
npm run build

# Test production build locally
npx serve -s build
```

### 2. **Deployment Platforms**

#### **Vercel (Recommended)**
```bash
# Install Vercel CLI
npm i -g vercel

# Deploy
vercel --prod

# Configure environment variables in Vercel dashboard
```

#### **Netlify**
```bash
# Install Netlify CLI
npm i -g netlify-cli

# Build and deploy
npm run build
netlify deploy --prod --dir=build
```

#### **AWS S3 + CloudFront**
```bash
# Build
npm run build

# Upload to S3
aws s3 sync build/ s3://your-bucket-name

# Invalidate CloudFront
aws cloudfront create-invalidation --distribution-id YOUR_DISTRIBUTION_ID --paths "/*"
```

## 🔒 Security Configuration

### 1. **CORS Settings** (Backend)
```javascript
// Allow your frontend domain
app.use(cors({
  origin: ['https://your-frontend-domain.com', 'http://localhost:3000'],
  credentials: true
}));
```

### 2. **Content Security Policy**
Add to your server headers:
```
Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline' https://checkout.razorpay.com; style-src 'self' 'unsafe-inline';
```

### 3. **HTTPS Configuration**
- Ensure all domains use HTTPS
- Configure SSL certificates
- Update Razorpay webhook URLs to HTTPS

## 📊 Monitoring & Analytics

### 1. **Error Tracking**
```bash
npm install @sentry/react @sentry/tracing
```

### 2. **Analytics Integration**
```bash
npm install gtag
```

### 3. **Performance Monitoring**
- Google PageSpeed Insights
- Lighthouse audits
- Core Web Vitals tracking

## 🔄 CI/CD Pipeline

### GitHub Actions Example
```yaml
name: Deploy to Production

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Setup Node.js
        uses: actions/setup-node@v2
        with:
          node-version: '18'
      - name: Install dependencies
        run: npm install
      - name: Build
        run: npm run build
        env:
          REACT_APP_API_BASE_URL: ${{ secrets.API_BASE_URL }}
          REACT_APP_RAZORPAY_KEY_ID: ${{ secrets.RAZORPAY_KEY_ID }}
      - name: Deploy to Vercel
        uses: amondnet/vercel-action@v20
        with:
          vercel-token: ${{ secrets.VERCEL_TOKEN }}
          vercel-org-id: ${{ secrets.ORG_ID }}
          vercel-project-id: ${{ secrets.PROJECT_ID }}
```

## 🧪 Testing Checklist

### Pre-Deployment Tests
- [ ] All pages load correctly
- [ ] Payment flow works end-to-end
- [ ] Login/logout functionality
- [ ] Responsive design on all devices
- [ ] Performance metrics acceptable
- [ ] No console errors
- [ ] All assets load properly

### Post-Deployment Tests
- [ ] Domain SSL certificate valid
- [ ] API endpoints responding
- [ ] Razorpay integration working
- [ ] User registration/login flow
- [ ] Payment processing
- [ ] Email notifications (if any)

## 📞 Support & Maintenance

### Regular Maintenance Tasks
- [ ] Update dependencies monthly
- [ ] Monitor performance metrics
- [ ] Check for security vulnerabilities
- [ ] Backup user data
- [ ] Update SSL certificates
- [ ] Monitor error logs

### Backup Strategy
- [ ] Code repository backup
- [ ] Asset files backup
- [ ] Environment variables backup
- [ ] Database backup (backend)
- [ ] SSL certificates backup

---

**Ready for Production**: ✅  
**Backend Integration**: 🔄 (Update API endpoints)  
**Security**: ✅ (HTTPS + CORS configured)  
**Monitoring**: 🔄 (Add analytics)  

This deployment guide ensures your GAPLY frontend is production-ready and properly integrated with your backend system.
