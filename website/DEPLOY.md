# Deploying to Vercel

This website is configured to deploy to Vercel at `https://dotstate.serkan.dev`.

## Configuration

Vercel is configured to use the `website` directory as the root directory. This can be set in two ways:

### Option 1: Vercel Dashboard (Recommended)

1. Go to your Vercel project settings
2. Navigate to **Settings** → **General**
3. Set **Root Directory** to `website`
4. **Disable Automatic Deployments** for the `main` branch (to respect versioning)
5. Save

### Option 2: Root-level vercel.json

A `vercel.json` file exists at the repository root that specifies `website` as the root directory. Vercel will automatically detect this.

## Deployment Strategy

**Important**: The website is configured to NOT auto-deploy on every commit to main. Deployments happen:

1. **On GitHub Releases**: When you create a release, the `deploy-website.yml` workflow automatically deploys
2. **Manual via GitHub Actions**: Use the "Deploy Website" workflow with a version tag
3. **Manual via Vercel CLI**: `vercel --prod` from the website directory

This ensures versioning is respected and the site only updates when you're ready.

## Deployment Steps

### Automatic (Recommended)

1. Create a GitHub Release:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```
2. Go to GitHub → Releases → Draft a new release
3. Select the tag and publish
4. The `deploy-website.yml` workflow will automatically deploy

### Manual via Vercel CLI

```bash
# From repository root
cd website
vercel --prod
```

### Manual via GitHub Actions

1. Go to Actions → "Deploy Website"
2. Click "Run workflow"
3. Enter version tag (e.g., `v1.0.0`)
4. Run

## Required Secrets

For GitHub Actions deployment, you need these secrets in your repository:

- `VERCEL_TOKEN`: Your Vercel API token (get from vercel.com/account/tokens)
- `VERCEL_ORG_ID`: Your Vercel organization ID (found in project settings)
- `VERCEL_PROJECT_ID`: Your Vercel project ID (found in project settings)

## Files

- `website/vercel.json` - Vercel configuration for the website
- `website/install.sh` - Installation script (served at `/install.sh`)
- `website/_headers` - HTTP headers configuration
- `website/sitemap.xml` - SEO sitemap
- `website/robots.txt` - Search engine crawler instructions
- `.github/workflows/deploy-website.yml` - GitHub Actions deployment workflow

## Domain Configuration

The domain `dotstate.serkan.dev` should be configured in:
- Vercel Dashboard → Project Settings → Domains
- Add the domain and follow DNS setup instructions

## Install Script

The install script is available at `https://dotstate.serkan.dev/install.sh` and is automatically deployed with the website. It installs DotState via Cargo.
