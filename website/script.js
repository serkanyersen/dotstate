// Update page title when switching sections for better SEO
function updatePageTitle(sectionName) {
    const baseTitle = 'DotState - Modern Dotfile Manager Built with Rust';
    const sectionTitles = {
        'overview': 'DotState - Modern Dotfile Manager | Overview',
        'features': 'DotState Features - Complete Dotfile Management Solution',
        'installation': 'Install DotState - Installation Guide',
        'quickstart': 'DotState Quick Start - Get Started in Minutes',
        'cli': 'DotState CLI Commands - Complete Command Reference',
        'examples': 'DotState Examples - Common Use Cases',
        'security': 'DotState Security - Secure Dotfile Management',
        'contributing': 'Contributing to DotState - Open Source'
    };
    document.title = sectionTitles[sectionName] || baseTitle;
}

// Menu Navigation
document.addEventListener('DOMContentLoaded', () => {
    const menuItems = document.querySelectorAll('.menu-item');
    const contentSections = document.querySelectorAll('.content-section');
    const copyButtons = document.querySelectorAll('.copy-btn');

    // Menu item click handlers
    menuItems.forEach(item => {
        item.addEventListener('click', () => {
            const sectionId = item.getAttribute('data-section');

            // Update active menu item
            menuItems.forEach(mi => mi.classList.remove('active'));
            item.classList.add('active');

            // Update active content section
            contentSections.forEach(section => section.classList.remove('active'));
            const targetSection = document.getElementById(sectionId);
            if (targetSection) {
                targetSection.classList.add('active');

                // Update page title for SEO
                updatePageTitle(sectionId);

                // Update URL without reload (for better UX)
                if (history.pushState) {
                    history.pushState(null, null, '#' + sectionId);
                }

                // Scroll to top of content and window
                const contentArea = document.querySelector('.content');
                if (contentArea) {
                    contentArea.scrollTop = 0;
                }
                window.scrollTo({ top: 0, behavior: 'smooth' });
            }
        });
    });

    // Keyboard navigation
    let currentIndex = 0;
    document.addEventListener('keydown', (e) => {
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();

            if (e.key === 'ArrowDown') {
                currentIndex = (currentIndex + 1) % menuItems.length;
            } else {
                currentIndex = (currentIndex - 1 + menuItems.length) % menuItems.length;
            }

            menuItems[currentIndex].click();
            menuItems[currentIndex].scrollIntoView({ behavior: 'smooth', block: 'nearest' });

            // Reset scroll to top when navigating
            const contentArea = document.querySelector('.content');
            if (contentArea) {
                contentArea.scrollTop = 0;
            }
            window.scrollTo({ top: 0, behavior: 'smooth' });
        } else if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            if (document.activeElement.classList.contains('menu-item')) {
                document.activeElement.click();
            }
        }
    });

    // Install widget tab functionality
    const installTabs = document.querySelectorAll('.install-tab');
    const installTabContents = document.querySelectorAll('.install-tab-content');

    installTabs.forEach(tab => {
        tab.addEventListener('click', () => {
            const targetTab = tab.getAttribute('data-tab');

            // Update active tab
            installTabs.forEach(t => t.classList.remove('active'));
            tab.classList.add('active');

            // Update active content
            installTabContents.forEach(content => content.classList.remove('active'));
            const targetContent = document.getElementById(`install-${targetTab}`);
            if (targetContent) {
                targetContent.classList.add('active');
            }

            // Handle "more" tab - navigate to installation section
            if (targetTab === 'more') {
                // Small delay to show the content, then navigate
                setTimeout(() => {
                    const installMenuItem = document.querySelector('[data-section="installation"]');
                    if (installMenuItem) {
                        installMenuItem.click();
                    }
                }, 300);
            }
        });
    });

    // Copy button functionality
    copyButtons.forEach(btn => {
        btn.addEventListener('click', async () => {
            const textToCopy = btn.getAttribute('data-copy');

            try {
                await navigator.clipboard.writeText(textToCopy);

                // Visual feedback
                const originalText = btn.textContent;
                btn.textContent = '✓';
                btn.classList.add('copied');

                setTimeout(() => {
                    btn.textContent = originalText;
                    btn.classList.remove('copied');
                }, 2000);
            } catch (err) {
                console.error('Failed to copy:', err);
                // Fallback for older browsers
                const textArea = document.createElement('textarea');
                textArea.value = textToCopy;
                textArea.style.position = 'fixed';
                textArea.style.opacity = '0';
                document.body.appendChild(textArea);
                textArea.select();
                try {
                    document.execCommand('copy');
                    btn.textContent = '✓';
                    setTimeout(() => {
                        btn.textContent = '📋';
                    }, 2000);
                } catch (err) {
                    console.error('Fallback copy failed:', err);
                }
                document.body.removeChild(textArea);
            }
        });
    });

    // Smooth scroll for anchor links
    document.querySelectorAll('a[href^="#"]').forEach(anchor => {
        anchor.addEventListener('click', function (e) {
            e.preventDefault();
            const target = document.querySelector(this.getAttribute('href'));
            if (target) {
                target.scrollIntoView({ behavior: 'smooth', block: 'start' });
            }
        });
    });

    // Initialize: show first section or section from hash
    const hash = window.location.hash.substring(1);
    if (hash) {
        const hashSection = document.getElementById(hash);
        if (hashSection) {
            const hashMenuItem = document.querySelector(`[data-section="${hash}"]`);
            if (hashMenuItem) {
                hashMenuItem.click();
            }
        }
    } else if (contentSections.length > 0) {
        contentSections[0].classList.add('active');
        updatePageTitle('overview');
    }

    // Handle browser back/forward buttons
    window.addEventListener('popstate', () => {
        const hash = window.location.hash.substring(1);
        if (hash) {
            const hashMenuItem = document.querySelector(`[data-section="${hash}"]`);
            if (hashMenuItem) {
                hashMenuItem.click();
            }
        }
    });
});

// Add smooth transitions
document.addEventListener('DOMContentLoaded', () => {
    // Add fade-in animation to page load
    document.body.style.opacity = '0';
    setTimeout(() => {
        document.body.style.transition = 'opacity 0.3s ease-in';
        document.body.style.opacity = '1';
    }, 10);
});

