# Code Signing and Notarization Setup

To enable code signing and notarization for macOS releases, you need to set up the following GitHub repository secrets:

## Required Secrets

### Apple Developer Certificate
- **`APPLE_CERTIFICATE_BASE64`**: Your Apple Developer certificate exported as a .p12 file and base64 encoded
- **`APPLE_CERTIFICATE_PASSWORD`**: The password for your .p12 certificate file
- **`APPLE_SIGNING_IDENTITY`**: The signing identity (usually your certificate's Common Name, e.g., "Developer ID Application: Your Name (TEAM_ID)")

### Apple ID for Notarization
- **`APPLE_ID`**: Your Apple ID email address
- **`APPLE_ID_PASSWORD`**: An app-specific password for your Apple ID (not your regular password)
- **`APPLE_TEAM_ID`**: Your Apple Developer Team ID

## Setup Instructions

### 1. Export Your Certificate
1. Open Keychain Access on macOS
2. Find your "Developer ID Application" certificate
3. Right-click and select "Export"
4. Save as .p12 format with a password
5. Convert to base64: `base64 -i certificate.p12 | pbcopy`

### 2. Create App-Specific Password
1. Go to https://appleid.apple.com
2. Sign in and go to "Security" section
3. Generate an app-specific password
4. Use this password (not your Apple ID password) for `APPLE_ID_PASSWORD`

### 3. Find Your Team ID
1. Go to https://developer.apple.com/account
2. Your Team ID is displayed in the top right corner

### 4. Add Secrets to GitHub
1. Go to your repository Settings > Secrets and variables > Actions
2. Add each secret with the exact names listed above

## Notes
- You need an active Apple Developer Program membership ($99/year)
- The certificate must be a "Developer ID Application" certificate (for distribution outside the App Store)
- Notarization can take several minutes to complete