# DotState Website

A fully functional website inspired by the DotState TUI design, featuring a dark theme with teal accents, interactive navigation, and comprehensive documentation.

## Files

- `index.html` - Main HTML structure with all content sections
- `styles.css` - Complete styling matching the TUI aesthetic
- `script.js` - Interactive functionality (menu navigation, copy buttons, keyboard shortcuts)

## Features

### Design
- Dark theme matching the TUI design perfectly
- Responsive layout (mobile-friendly)
- Teal/cyan borders and accents
- Terminal-inspired typography (JetBrains Mono)
- Smooth animations and transitions

### Functionality
- **Interactive Menu Navigation**: Click menu items to switch sections
- **Keyboard Navigation**: Use ↑↓ arrow keys to navigate, Enter to select
- **Copy to Clipboard**: Click 📋 buttons to copy code snippets
- **Smooth Scrolling**: Smooth transitions between sections
- **Active Section Highlighting**: Current section is highlighted in green

### Content Sections
1. **Overview** - What is DotState and why use it
2. **Features** - Comprehensive feature list
3. **Installation** - Multiple installation methods
4. **Quick Start** - Step-by-step getting started guide
5. **CLI Commands** - Complete CLI reference with examples
6. **Examples** - Common workflows and use cases
7. **Security** - Security considerations and best practices
8. **Contributing** - How to contribute to the project

## Usage

### Local Development

Simply open `index.html` in a web browser, or serve it with any static file server:

```bash
# Using Python
cd website
python3 -m http.server 8000

# Using Node.js (http-server)
cd website
npx http-server

# Using PHP
cd website
php -S localhost:8000
```

Then visit `http://localhost:8000` in your browser.

### Deployment

The website is deployed to **Vercel** at: **https://dotstate.serkan.dev**

#### Deploying to Vercel

1. **Via Vercel CLI**:
   ```bash
   cd website
   vercel
   ```

2. **Via GitHub Integration**:
   - Connect your repository to Vercel
   - Set root directory to `website`
   - Deploy automatically on push

3. **Via Vercel Dashboard**:
   - Import your repository
   - Set root directory to `website`
   - Deploy

The website is also ready to deploy to:
- **GitHub Pages**: Just push to a `gh-pages` branch
- **Netlify**: Drag and drop the `website` folder
- **Any static hosting**: Upload the `website` folder

## Keyboard Shortcuts

- `↑` / `↓` - Navigate menu items
- `Enter` / `Space` - Select menu item
- Click menu items - Switch sections
- Click 📋 buttons - Copy code to clipboard

## Customization

The design uses CSS variables for easy theming. Modify the `:root` variables in `styles.css` to change colors:

```css
:root {
    --bg-primary: #000000;
    --border-color: #00d4ff;
    --text-green: #00ff00;
    --text-purple: #d946ef;
    /* ... */
}
```

## Browser Support

- Modern browsers (Chrome, Firefox, Safari, Edge)
- Requires JavaScript enabled
- CSS Grid and Flexbox support recommended

