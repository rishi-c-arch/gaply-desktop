# GAPLY Frontend - Backend Integration Guide

## 🎯 Integration with Go Backend + Supabase

This guide provides everything you need to integrate your GAPLY frontend with your Go backend and Supabase database.

---

## 🚀 Quick Integration Checklist

### ✅ **Frontend Updates Required**
- [ ] Update API endpoints in `src/config/api.ts`
- [ ] Configure environment variables
- [ ] Test authentication flow
- [ ] Verify payment integration
- [ ] Test error handling

### ✅ **Backend Requirements**
- [ ] Implement Go API endpoints
- [ ] Set up Supabase database schema
- [ ] Configure CORS settings
- [ ] Implement JWT authentication
- [ ] Set up Razorpay webhooks

---

## 🔧 Go Backend API Endpoints

### **Required Endpoints Structure**

```go
// main.go - Your Go backend structure
package main

import (
    "github.com/gin-gonic/gin"
    "github.com/golang-jwt/jwt/v5"
    "github.com/supabase-community/supabase-go"
)

// API Routes
func setupRoutes(r *gin.Engine) {
    api := r.Group("/api/v1")
    
    // Authentication routes
    auth := api.Group("/auth")
    {
        auth.POST("/login", loginHandler)
        auth.POST("/register", registerHandler)
        auth.POST("/refresh", refreshTokenHandler)
        auth.POST("/logout", logoutHandler)
        auth.GET("/verify", verifyTokenHandler)
    }
    
    // User routes
    users := api.Group("/user")
    users.Use(authMiddleware()) // JWT middleware
    {
        users.GET("/profile", getUserProfileHandler)
        users.PUT("/profile", updateUserProfileHandler)
        users.GET("/account", getUserAccountHandler)
        users.PUT("/package", updateUserPackageHandler)
        users.GET("/usage", getUserUsageHandler)
    }
    
    // Payment routes
    payments := api.Group("/payment")
    payments.Use(authMiddleware())
    {
        payments.POST("/create-order", createOrderHandler)
        payments.POST("/verify", verifyPaymentHandler)
        payments.POST("/webhook", razorpayWebhookHandler)
        payments.GET("/history", getPaymentHistoryHandler)
    }
    
    // Package routes
    packages := api.Group("/packages")
    {
        packages.GET("/", getPackagesHandler)
        packages.GET("/:id", getPackageDetailsHandler)
    }
}

// CORS Configuration
func setupCORS(r *gin.Engine) {
    r.Use(func(c *gin.Context) {
        c.Header("Access-Control-Allow-Origin", "https://your-frontend-domain.com")
        c.Header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        c.Header("Access-Control-Allow-Headers", "Origin, Content-Type, Authorization")
        c.Header("Access-Control-Allow-Credentials", "true")
        
        if c.Request.Method == "OPTIONS" {
            c.AbortWithStatus(204)
            return
        }
        
        c.Next()
    })
}
```

### **Authentication Handlers**

```go
// handlers/auth.go
package handlers

import (
    "github.com/gin-gonic/gin"
    "github.com/golang-jwt/jwt/v5"
    "time"
)

type LoginRequest struct {
    Email    string `json:"email" binding:"required,email"`
    Password string `json:"password" binding:"required"`
}

type RegisterRequest struct {
    Email    string `json:"email" binding:"required,email"`
    Password string `json:"password" binding:"required,min=8"`
    Name     string `json:"name" binding:"required"`
}

type AuthResponse struct {
    Token string `json:"token"`
    User  User   `json:"user"`
}

type User struct {
    ID        string    `json:"id"`
    Email     string    `json:"email"`
    Name      string    `json:"name"`
    Package   string    `json:"package"`
    ValidUntil time.Time `json:"valid_until"`
    CreatedAt time.Time `json:"created_at"`
    UpdatedAt time.Time `json:"updated_at"`
}

func loginHandler(c *gin.Context) {
    var req LoginRequest
    if err := c.ShouldBindJSON(&req); err != nil {
        c.JSON(400, gin.H{"error": "Invalid request"})
        return
    }
    
    // Verify user credentials with Supabase
    user, err := verifyUserCredentials(req.Email, req.Password)
    if err != nil {
        c.JSON(401, gin.H{"error": "Invalid credentials"})
        return
    }
    
    // Generate JWT token
    token, err := generateJWTToken(user.ID)
    if err != nil {
        c.JSON(500, gin.H{"error": "Failed to generate token"})
        return
    }
    
    c.JSON(200, AuthResponse{
        Token: token,
        User:  user,
    })
}

func registerHandler(c *gin.Context) {
    var req RegisterRequest
    if err := c.ShouldBindJSON(&req); err != nil {
        c.JSON(400, gin.H{"error": "Invalid request"})
        return
    }
    
    // Create user in Supabase
    user, err := createUser(req.Email, req.Password, req.Name)
    if err != nil {
        c.JSON(400, gin.H{"error": "Failed to create user"})
        return
    }
    
    // Generate JWT token
    token, err := generateJWTToken(user.ID)
    if err != nil {
        c.JSON(500, gin.H{"error": "Failed to generate token"})
        return
    }
    
    c.JSON(201, AuthResponse{
        Token: token,
        User:  user,
    })
}

func generateJWTToken(userID string) (string, error) {
    claims := jwt.MapClaims{
        "user_id": userID,
        "exp":     time.Now().Add(time.Hour * 24 * 7).Unix(), // 7 days
        "iat":     time.Now().Unix(),
    }
    
    token := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
    return token.SignedString([]byte("your-secret-key")) // Use environment variable
}
```

### **Payment Handlers**

```go
// handlers/payment.go
package handlers

import (
    "github.com/gin-gonic/gin"
    "github.com/razorpay/razorpay-go"
)

type CreateOrderRequest struct {
    PackageID string `json:"package_id" binding:"required"`
}

type PaymentOrder struct {
    ID               string  `json:"id"`
    RazorpayOrderID  string  `json:"razorpay_order_id"`
    UserID           string  `json:"user_id"`
    PackageType      string  `json:"package_type"`
    Amount           int     `json:"amount"`
    Status           string  `json:"status"`
    CreatedAt        time.Time `json:"created_at"`
}

func createOrderHandler(c *gin.Context) {
    userID := c.GetString("user_id") // From JWT middleware
    var req CreateOrderRequest
    
    if err := c.ShouldBindJSON(&req); err != nil {
        c.JSON(400, gin.H{"error": "Invalid request"})
        return
    }
    
    // Get package details
    packageDetails, err := getPackageByID(req.PackageID)
    if err != nil {
        c.JSON(404, gin.H{"error": "Package not found"})
        return
    }
    
    // Create Razorpay order
    razorpayClient := razorpay.NewClient("rzp_live_REMOVED", "RAZORPAY_SECRET_REMOVED")
    
    orderData := map[string]interface{}{
        "amount":   packageDetails.Price * 100, // Convert to paise
        "currency": "INR",
        "receipt":  "order_" + generateOrderID(),
    }
    
    order, err := razorpayClient.Order.Create(orderData, nil)
    if err != nil {
        c.JSON(500, gin.H{"error": "Failed to create order"})
        return
    }
    
    // Save order to database
    paymentOrder := PaymentOrder{
        ID:              generateOrderID(),
        RazorpayOrderID: order["id"].(string),
        UserID:          userID,
        PackageType:     packageDetails.Type,
        Amount:          packageDetails.Price,
        Status:          "pending",
        CreatedAt:       time.Now(),
    }
    
    err = savePaymentOrder(paymentOrder)
    if err != nil {
        c.JSON(500, gin.H{"error": "Failed to save order"})
        return
    }
    
    c.JSON(200, gin.H{
        "id":               paymentOrder.ID,
        "razorpay_order_id": paymentOrder.RazorpayOrderID,
        "amount":           paymentOrder.Amount,
        "currency":         "INR",
        "package_type":     paymentOrder.PackageType,
        "status":           paymentOrder.Status,
        "created_at":       paymentOrder.CreatedAt,
    })
}

func verifyPaymentHandler(c *gin.Context) {
    userID := c.GetString("user_id")
    var req struct {
        RazorpayOrderID   string `json:"razorpay_order_id"`
        RazorpayPaymentID string `json:"razorpay_payment_id"`
        RazorpaySignature string `json:"razorpay_signature"`
    }
    
    if err := c.ShouldBindJSON(&req); err != nil {
        c.JSON(400, gin.H{"error": "Invalid request"})
        return
    }
    
    // Verify Razorpay signature
    isValid, err := verifyRazorpaySignature(req.RazorpayOrderID, req.RazorpayPaymentID, req.RazorpaySignature)
    if err != nil || !isValid {
        c.JSON(400, gin.H{"error": "Invalid payment signature"})
        return
    }
    
    // Update order status
    err = updatePaymentOrderStatus(req.RazorpayOrderID, "completed")
    if err != nil {
        c.JSON(500, gin.H{"error": "Failed to update order"})
        return
    }
    
    // Update user package
    order, err := getPaymentOrderByRazorpayID(req.RazorpayOrderID)
    if err != nil {
        c.JSON(500, gin.H{"error": "Order not found"})
        return
    }
    
    err = updateUserPackage(userID, order.PackageType)
    if err != nil {
        c.JSON(500, gin.H{"error": "Failed to update user package"})
        return
    }
    
    c.JSON(200, gin.H{
        "success": true,
        "package": order.PackageType,
    })
}
```

---

## 🗄️ Supabase Database Schema

### **Required Tables**

```sql
-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    name VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- User packages table
CREATE TABLE user_packages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    package_type VARCHAR(50) NOT NULL,
    status VARCHAR(20) DEFAULT 'active',
    valid_until TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Usage tracking table
CREATE TABLE user_usage (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    feature VARCHAR(50) NOT NULL,
    uses_remaining INTEGER DEFAULT 0,
    last_used TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(user_id, feature)
);

-- Payment orders table
CREATE TABLE payment_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    razorpay_order_id VARCHAR(255) UNIQUE,
    package_type VARCHAR(50),
    amount INTEGER,
    status VARCHAR(20) DEFAULT 'pending',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Packages table
CREATE TABLE packages (
    id VARCHAR(50) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    price INTEGER NOT NULL,
    description TEXT,
    features JSONB,
    popular BOOLEAN DEFAULT FALSE,
    valid_days INTEGER DEFAULT 365,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Insert default packages
INSERT INTO packages (id, name, price, description, features, popular, valid_days) VALUES
('basic', 'Gaply Basic', 47100, 'Research Gap Finder - Analyze 5 base papers to identify research gaps and opportunities', 
 '["Research Gap Finder (1 use)", "Analyze 5 base papers", "Publishable problem statement", "Clear objectives & hypotheses", "Valid for 365 days"]', 
 false, 365),
('plus', 'Gaply Plus', 106100, 'Research Pro - Gap Finder (2 uses) + Deep Paper Analysis (1 use) with 70+ parameter evaluation',
 '["Research Gap Finder (2 uses)", "Deep Paper Analysis (1 use)", "70+ parameter evaluation", "Journal compliance checking", "Valid for 365 days"]',
 true, 365),
('pro', 'Gaply Pro', 224100, 'Research Elite - Full access (5 uses each) + Team support with 30min session + WhatsApp support',
 '["Research Gap Finder (5 uses)", "Deep Paper Analysis (5 uses)", "Team support (30min session)", "WhatsApp support", "Valid for 365 days"]',
 false, 365);

-- Create indexes for better performance
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_user_packages_user_id ON user_packages(user_id);
CREATE INDEX idx_user_usage_user_id ON user_usage(user_id);
CREATE INDEX idx_payment_orders_user_id ON payment_orders(user_id);
CREATE INDEX idx_payment_orders_razorpay_id ON payment_orders(razorpay_order_id);

-- Enable Row Level Security (RLS)
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_packages ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_usage ENABLE ROW LEVEL SECURITY;
ALTER TABLE payment_orders ENABLE ROW LEVEL SECURITY;

-- Create RLS policies
CREATE POLICY "Users can view own data" ON users FOR SELECT USING (auth.uid() = id);
CREATE POLICY "Users can update own data" ON users FOR UPDATE USING (auth.uid() = id);

CREATE POLICY "Users can view own packages" ON user_packages FOR SELECT USING (auth.uid() = user_id);
CREATE POLICY "Users can view own usage" ON user_usage FOR SELECT USING (auth.uid() = user_id);
CREATE POLICY "Users can view own orders" ON payment_orders FOR SELECT USING (auth.uid() = user_id);
```

---

## 🔧 Frontend Integration Steps

### **1. Update API Configuration**

```typescript
// src/config/api.ts - Update with your backend URL
export const API_CONFIG = {
  BASE_URL: process.env.REACT_APP_API_BASE_URL || 'https://your-go-backend.com',
  // ... rest of config
};
```

### **2. Environment Variables**

```bash
# .env.production
REACT_APP_API_BASE_URL=https://your-go-backend.com
REACT_APP_RAZORPAY_KEY_ID=rzp_live_REMOVED
REACT_APP_RAZORPAY_KEY_SECRET=RAZORPAY_SECRET_REMOVED
REACT_APP_ENVIRONMENT=production
```

### **3. Test Integration**

```bash
# Test your integration
npm start
# Navigate to login page and test authentication
# Test package selection and payment flow
# Verify error handling
```

---

## 🚀 Deployment Checklist

### **Backend Deployment**
- [ ] Deploy Go backend to your hosting platform
- [ ] Configure environment variables
- [ ] Set up SSL certificates
- [ ] Configure CORS settings
- [ ] Test all API endpoints

### **Database Setup**
- [ ] Set up Supabase project
- [ ] Run database schema migrations
- [ ] Insert default packages
- [ ] Configure RLS policies
- [ ] Test database connections

### **Frontend Deployment**
- [ ] Update API endpoints
- [ ] Configure environment variables
- [ ] Build production version
- [ ] Deploy to hosting platform
- [ ] Test end-to-end functionality

---

## 🧪 Testing Your Integration

### **API Testing**
```bash
# Test authentication
curl -X POST https://your-backend.com/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"test@example.com","password":"password123"}'

# Test package listing
curl -X GET https://your-backend.com/api/v1/packages \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

### **Frontend Testing**
1. **Authentication Flow**
   - Register new user
   - Login with credentials
   - Verify JWT token handling
   - Test logout functionality

2. **Package Selection**
   - Load package list
   - Select package
   - Create payment order
   - Complete payment flow

3. **Account Management**
   - View user account
   - Check usage statistics
   - Verify package details

---

## 🔒 Security Considerations

### **JWT Security**
- Use strong secret keys
- Implement token refresh
- Set appropriate expiration times
- Validate tokens on every request

### **Payment Security**
- Verify Razorpay signatures
- Use HTTPS for all requests
- Validate payment amounts
- Log all payment transactions

### **API Security**
- Implement rate limiting
- Validate all inputs
- Use parameterized queries
- Enable CORS properly

---

## 📞 Support & Troubleshooting

### **Common Issues**
1. **CORS Errors**: Check CORS configuration in Go backend
2. **Authentication Failures**: Verify JWT secret and token format
3. **Payment Issues**: Check Razorpay webhook configuration
4. **Database Errors**: Verify Supabase connection and RLS policies

### **Debugging**
- Check browser network tab for API calls
- Verify environment variables are set
- Check backend logs for errors
- Test API endpoints with Postman/curl

---

**Your GAPLY frontend is now ready for seamless integration with your Go backend and Supabase database!** 🚀

The integration provides:
- ✅ Secure JWT authentication
- ✅ Razorpay payment processing
- ✅ Real-time user account management
- ✅ Comprehensive error handling
- ✅ Production-ready architecture
