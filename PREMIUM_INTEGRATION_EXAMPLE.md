# 🔗 Premium Features Integration Example

## How to Integrate Premium Features with Your Existing Components

This guide shows you how to add premium feature access control to your existing `FreeFeatures3D.tsx` component.

---

## 📝 **Step 1: Import Authentication Context**

Add this import to your `FreeFeatures3D.tsx`:

```tsx
import { useAuth } from '../contexts/AuthContext';
```

---

## 📝 **Step 2: Add Authentication State**

Inside your component, add authentication state:

```tsx
const FreeFeatures3D: React.FC = () => {
  // ... existing state ...
  
  // Add authentication
  const { user, isAuthenticated, canUseFeature, subscription } = useAuth();
  
  // ... rest of your component
};
```

---

## 📝 **Step 3: Add Premium Feature Buttons**

Add premium feature buttons to your existing UI. Here's an example:

```tsx
// Add this to your existing tool cards section
<div style={{
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
  gap: '20px',
  marginTop: '40px'
}}>
  {/* Existing Free Features */}
  {/* ... your existing Academic AI Remover, Paper Search, Journal Matching ... */}
  
  {/* New Premium Features */}
  <div style={{
    background: 'linear-gradient(135deg, #667eea, #764ba2)',
    borderRadius: '20px',
    padding: '24px',
    color: 'white',
    textAlign: 'center',
    boxShadow: '0 10px 30px rgba(102, 126, 234, 0.3)'
  }}>
    <h3 style={{
      fontSize: '20px',
      fontWeight: '600',
      marginBottom: '12px',
      fontFamily: "'Space Grotesk', sans-serif"
    }}>
      🔍 Advanced Literature Analysis
    </h3>
    <p style={{
      fontSize: '14px',
      opacity: 0.9,
      marginBottom: '20px',
      fontFamily: "'Inter', sans-serif"
    }}>
      Upload up to 5 research papers and get comprehensive literature gap analysis
    </p>
    
    {!isAuthenticated ? (
      <button
        onClick={() => {
          // Redirect to login
          window.location.href = '/login';
        }}
        style={{
          background: 'rgba(255, 255, 255, 0.2)',
          border: '1px solid rgba(255, 255, 255, 0.3)',
          borderRadius: '12px',
          padding: '12px 24px',
          color: 'white',
          fontSize: '14px',
          fontWeight: '600',
          cursor: 'pointer',
          transition: 'all 0.3s ease',
          fontFamily: "'Inter', sans-serif"
        }}
      >
        Login to Access
      </button>
    ) : (
      <button
        onClick={handleLiteratureAnalysis}
        style={{
          background: 'rgba(255, 255, 255, 0.2)',
          border: '1px solid rgba(255, 255, 255, 0.3)',
          borderRadius: '12px',
          padding: '12px 24px',
          color: 'white',
          fontSize: '14px',
          fontWeight: '600',
          cursor: 'pointer',
          transition: 'all 0.3s ease',
          fontFamily: "'Inter', sans-serif"
        }}
      >
        {subscription?.remaining_uses?.gap_finder || 0} Uses Remaining
      </button>
    )}
  </div>

  <div style={{
    background: 'linear-gradient(135deg, #48bb78, #38a169)',
    borderRadius: '20px',
    padding: '24px',
    color: 'white',
    textAlign: 'center',
    boxShadow: '0 10px 30px rgba(72, 187, 120, 0.3)'
  }}>
    <h3 style={{
      fontSize: '20px',
      fontWeight: '600',
      marginBottom: '12px',
      fontFamily: "'Space Grotesk', sans-serif"
    }}>
      📊 Deep Paper Analysis
    </h3>
    <p style={{
      fontSize: '14px',
      opacity: 0.9,
      marginBottom: '20px',
      fontFamily: "'Inter', sans-serif"
    }}>
      70-parameter evaluation engine with professional HTML reports
    </p>
    
    {!isAuthenticated ? (
      <button
        onClick={() => {
          window.location.href = '/login';
        }}
        style={{
          background: 'rgba(255, 255, 255, 0.2)',
          border: '1px solid rgba(255, 255, 255, 0.3)',
          borderRadius: '12px',
          padding: '12px 24px',
          color: 'white',
          fontSize: '14px',
          fontWeight: '600',
          cursor: 'pointer',
          transition: 'all 0.3s ease',
          fontFamily: "'Inter', sans-serif"
        }}
      >
        Login to Access
      </button>
    ) : (
      <button
        onClick={handleDeepPaperAnalysis}
        style={{
          background: 'rgba(255, 255, 255, 0.2)',
          border: '1px solid rgba(255, 255, 255, 0.3)',
          borderRadius: '12px',
          padding: '12px 24px',
          color: 'white',
          fontSize: '14px',
          fontWeight: '600',
          cursor: 'pointer',
          transition: 'all 0.3s ease',
          fontFamily: "'Inter', sans-serif"
        }}
      >
        {subscription?.remaining_uses?.deep_eval || 0} Uses Remaining
      </button>
    )}
  </div>
</div>
```

---

## 📝 **Step 4: Add Premium Feature Handlers**

Add these handler functions to your component:

```tsx
// Add these functions inside your FreeFeatures3D component

const handleLiteratureAnalysis = async () => {
  if (!user) {
    alert('Please login to access premium features');
    return;
  }

  // Check feature access
  const access = await canUseFeature('gap_finder');
  if (!access.canUse) {
    alert(`You do not have access to this feature. ${access.error || 'Please purchase a plan.'}`);
    return;
  }

  // Create file input for paper uploads
  const input = document.createElement('input');
  input.type = 'file';
  input.multiple = true;
  input.accept = '.pdf';
  input.onchange = async (e) => {
    const files = Array.from((e.target as HTMLInputElement).files || []);
    if (files.length === 0) return;
    
    if (files.length > 5) {
      alert('You can upload a maximum of 5 papers');
      return;
    }

    try {
      // Use premium service to process papers
      const result = await premiumService.useLiteratureAnalysis(files);
      
      if (result.success) {
        alert('Literature analysis started! You will receive the report shortly.');
        // You can redirect to a results page or show a loading state
      } else {
        alert(result.error || 'Failed to start analysis');
      }
    } catch (error) {
      console.error('Literature analysis error:', error);
      alert('An error occurred while processing your request');
    }
  };
  
  input.click();
};

const handleDeepPaperAnalysis = async () => {
  if (!user) {
    alert('Please login to access premium features');
    return;
  }

  // Check feature access
  const access = await canUseFeature('deep_eval');
  if (!access.canUse) {
    alert(`You do not have access to this feature. ${access.error || 'Please purchase a plan.'}`);
    return;
  }

  // Create file input for PDF upload
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.pdf';
  input.onchange = async (e) => {
    const files = (e.target as HTMLInputElement).files;
    if (!files || files.length === 0) return;
    
    const file = files[0];

    try {
      // Use premium service to process paper
      const result = await premiumService.useDeepPaperAnalysis(file);
      
      if (result.success) {
        alert('Deep paper analysis started! You will receive the report shortly.');
        // You can redirect to a results page or show a loading state
      } else {
        alert(result.error || 'Failed to start analysis');
      }
    } catch (error) {
      console.error('Deep paper analysis error:', error);
      alert('An error occurred while processing your request');
    }
  };
  
  input.click();
};
```

---

## 📝 **Step 5: Add Premium Service Import**

Add this import at the top of your file:

```tsx
import { premiumService } from '../services/premiumService';
```

---

## 📝 **Step 6: Add User Status Display**

Add a user status display to your existing header:

```tsx
// Add this to your existing header section
{isAuthenticated && user && (
  <div style={{
    position: 'absolute',
    top: '20px',
    right: '20px',
    background: 'rgba(255, 255, 255, 0.9)',
    backdropFilter: 'blur(10px)',
    borderRadius: '12px',
    padding: '12px 16px',
    boxShadow: '0 4px 20px rgba(0, 0, 0, 0.1)',
    border: '1px solid rgba(255, 255, 255, 0.2)'
  }}>
    <div style={{
      display: 'flex',
      alignItems: 'center',
      gap: '8px',
      fontSize: '14px',
      fontFamily: "'Inter', sans-serif"
    }}>
      <div style={{
        width: '8px',
        height: '8px',
        borderRadius: '50%',
        background: user.is_premium ? '#48bb78' : '#a0aec0'
      }} />
      <span style={{ color: '#4a5568', fontWeight: '500' }}>
        {user.first_name} {user.last_name}
      </span>
      {user.is_premium && (
        <span style={{
          background: 'linear-gradient(135deg, #48bb78, #38a169)',
          color: 'white',
          padding: '2px 8px',
          borderRadius: '12px',
          fontSize: '10px',
          fontWeight: '600'
        }}>
          PREMIUM
        </span>
      )}
    </div>
  </div>
)}
```

---

## 📝 **Step 7: Add Navigation to Premium Page**

Add a "Get Premium" button to your existing navigation:

```tsx
// Add this to your existing navigation or header
{!isAuthenticated && (
  <button
    onClick={() => {
      window.location.href = '/login';
    }}
    style={{
      background: 'linear-gradient(135deg, #667eea, #764ba2)',
      color: 'white',
      border: 'none',
      borderRadius: '12px',
      padding: '12px 24px',
      fontSize: '14px',
      fontWeight: '600',
      cursor: 'pointer',
      transition: 'all 0.3s ease',
      fontFamily: "'Inter', sans-serif",
      boxShadow: '0 4px 15px rgba(102, 126, 234, 0.4)'
    }}
    onMouseOver={(e) => {
      e.currentTarget.style.transform = 'translateY(-2px)';
      e.currentTarget.style.boxShadow = '0 6px 20px rgba(102, 126, 234, 0.5)';
    }}
    onMouseOut={(e) => {
      e.currentTarget.style.transform = 'translateY(0)';
      e.currentTarget.style.boxShadow = '0 4px 15px rgba(102, 126, 234, 0.4)';
    }}
  >
    Get Premium
  </button>
)}

{isAuthenticated && !user.is_premium && (
  <button
    onClick={() => {
      window.location.href = '/premium';
    }}
    style={{
      background: 'linear-gradient(135deg, #48bb78, #38a169)',
      color: 'white',
      border: 'none',
      borderRadius: '12px',
      padding: '12px 24px',
      fontSize: '14px',
      fontWeight: '600',
      cursor: 'pointer',
      transition: 'all 0.3s ease',
      fontFamily: "'Inter', sans-serif",
      boxShadow: '0 4px 15px rgba(72, 187, 120, 0.4)'
    }}
    onMouseOver={(e) => {
      e.currentTarget.style.transform = 'translateY(-2px)';
      e.currentTarget.style.boxShadow = '0 6px 20px rgba(72, 187, 120, 0.5)';
    }}
    onMouseOut={(e) => {
      e.currentTarget.style.transform = 'translateY(0)';
      e.currentTarget.style.boxShadow = '0 4px 15px rgba(72, 187, 120, 0.4)';
    }}
  >
    Upgrade to Premium
  </button>
)}
```

---

## 🎉 **You're Done!**

Your existing `FreeFeatures3D.tsx` component now has:

✅ **Premium Feature Buttons** - Two new premium features with access control  
✅ **Authentication Integration** - Login status and user information display  
✅ **Usage Tracking** - Shows remaining uses for each feature  
✅ **Access Control** - Prevents unauthorized access to premium features  
✅ **Professional UI** - Beautiful gradient cards for premium features  

### What happens when users click premium features:

1. **Not Logged In**: Redirects to login page
2. **Logged In, No Access**: Shows error message and suggests purchasing a plan
3. **Has Access**: Opens file upload dialog and processes the request
4. **Usage Tracking**: Automatically decrements remaining uses

### Next Steps:

1. Test the integration with your existing component
2. Customize the styling to match your design
3. Add more premium features as needed
4. Set up the complete authentication system using the setup guide

---

**Need Help?** Check the `PREMIUM_FEATURES_SETUP_GUIDE.md` for complete setup instructions!
