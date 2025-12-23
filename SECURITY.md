# Security Policy

## Supported Versions

We actively support the following versions with security updates:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability, please **do not** open a public issue. Instead, please email security@yourdomain.com (or create a private security advisory on GitHub).

### What to Include

When reporting a vulnerability, please include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if you have one)

### Response Time

We aim to:
- Acknowledge receipt within 48 hours
- Provide an initial assessment within 7 days
- Keep you updated on progress

### Security Best Practices

DotState follows these security practices:
- No shell injection vulnerabilities (direct command execution)
- Path validation to prevent dangerous operations
- Secure token storage
- Automatic backups before destructive operations
- Git repository detection to prevent nested repos

## Security Considerations

### GitHub Tokens

- Tokens are stored in local config files
- Use tokens with minimal required permissions
- Rotate tokens regularly
- Never commit tokens to version control

### File Operations

- DotState validates paths before operations
- Dangerous paths (like home directory root) are blocked
- Automatic backups are created before file modifications
- Symlinks are validated before creation

### Package Installation

- Package names are never shell-escaped (direct args prevent injection)
- Custom packages use shell execution (user's responsibility)
- Sudo password detection before attempting installation

