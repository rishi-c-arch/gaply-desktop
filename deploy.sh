#!/bin/bash

# Build the React app
echo "Building React app..."
npm run build

# Copy build files to web deploy directory
echo "Deploying to web directory..."
cp -r build/* ../gaply-web-deploy/

echo "Deployment complete! The React app is now available in gaply-web-deploy/"
echo "You can serve it with: cd ../gaply-web-deploy && python3 -m http.server 8080"
