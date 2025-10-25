const { execSync } = require('child_process');
const fs = require('fs');

console.log('🚀 Starting deployment process...');

try {
  console.log('📦 Building the project...');
  execSync('npm run build', { stdio: 'inherit' });
  
  console.log('🚀 Deploying to Vercel...');
  execSync('npx vercel --prod --yes', { stdio: 'inherit' });
  
  console.log('✅ Deployment completed successfully!');
} catch (error) {
  console.error('❌ Deployment failed:', error.message);
  process.exit(1);
}
