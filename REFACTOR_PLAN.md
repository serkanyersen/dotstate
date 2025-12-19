# UI Overhaul & Refactoring Plan

## Priority Alignment (per instructions.md)

### Core Focus
1. **UI/UX is the differentiator** - "nice, friendly interface" is a key requirement
2. **Mouse support is essential** - "nice way to navigate the app, mouse support would be nice"
3. **Polish before features** - UI overhaul before moving to nice-to-haves

## Architecture Pattern Analysis

### Option 1: Component Architecture
**Pros:**
- Each screen (Welcome, MainMenu, GitHubAuth, etc.) becomes a reusable component
- Components manage their own state
- Easy to test individual components
- Good for modular UI elements (inputs, lists, buttons)

**Cons:**
- May require more boilerplate
- Component communication can be complex

### Option 2: Elm Architecture (Model-View-Update)
**Pros:**
- Clear separation: Model (state) → View (render) → Update (events)
- Unidirectional data flow
- Easy to reason about state changes
- Good for complex state management

**Cons:**
- All state in one place (can get large)
- Update function can become complex

### Option 3: Flux Architecture
**Pros:**
- Actions → Dispatcher → Store → View
- Very clear data flow
- Good for complex apps with many state updates

**Cons:**
- More boilerplate than needed for this app
- Overkill for current complexity

### **Decision: Component Architecture**
- Best fit for TUI with distinct screens
- Each screen is naturally a component
- Can share common UI elements (inputs, buttons, lists)
- Easier to maintain and test
- Aligns with ratatui's widget-based approach

## Implementation Plan

### Phase 1: Architecture Refactoring
1. **Create Component Trait**
   - `trait Component { fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()>; fn handle_event(&mut self, event: Event) -> Result<ComponentAction>; }`
   - `enum ComponentAction { None, Navigate(Screen), Quit, UpdateState }`

2. **Convert Screens to Components**
   - `WelcomeComponent`
   - `MainMenuComponent`
   - `GitHubAuthComponent`
   - `DotfileSelectionComponent`
   - `ViewSyncedFilesComponent`
   - `MessageComponent` (for Push/Pull)

3. **Shared UI Components**
   - `InputField` - reusable input with cursor
   - `Button` - clickable buttons
   - `ScrollableList` - list with mouse scroll support
   - `StatusBar` - footer with help text

### Phase 2: Mouse Support Implementation
1. **Mouse Event Handling**
   - Click detection on all interactive elements
   - Scroll wheel for lists
   - Drag for scrollbars
   - Click-to-focus for inputs

2. **Hit Testing**
   - Track widget positions during render
   - Store clickable areas in component state
   - Map mouse coordinates to actions

3. **Mouse Feedback**
   - Hover effects (if terminal supports)
   - Visual feedback on clickable elements

### Phase 3: UI Polish & Clear Widget
1. **Screen Clearing**
   - Use `Clear` widget at start of every screen render
   - Clear overlays before rendering
   - Clear popups before showing

2. **Visual Design Overhaul**
   - **Welcome Screen:**
     - Better welcome message with ASCII art or styled text
     - Clear call-to-action
     - Better spacing and layout

   - **Main Menu:**
     - Better visual hierarchy
     - Icons or symbols for menu items
     - Active profile indicator
     - Better borders and styling

   - **All Screens:**
     - Consistent header/footer design
     - Better color scheme
     - Improved spacing
     - Visual separators

3. **Status Indicators**
   - Better error message display
   - Loading indicators
   - Success/error states with colors

### Phase 4: Testing & Refinement
1. Test mouse interactions on all screens
2. Verify no background bleed-through
3. Test on different terminal sizes
4. Performance optimization

## File Structure After Refactoring

```
src/
├── main.rs
├── lib.rs
├── app.rs              # Main app, component orchestration
├── components/
│   ├── mod.rs
│   ├── component.rs    # Component trait
│   ├── welcome.rs
│   ├── main_menu.rs
│   ├── github_auth.rs
│   ├── dotfile_selection.rs
│   ├── synced_files.rs
│   └── message.rs
├── widgets/
│   ├── mod.rs
│   ├── input.rs        # Reusable input field
│   ├── button.rs       # Clickable button
│   ├── scrollable_list.rs
│   └── status_bar.rs
├── ui.rs               # Shared UI utilities, styles
├── config.rs
├── file_manager.rs
├── git.rs
├── github.rs
└── tui.rs
```

## Implementation Order

1. ✅ Create Component trait and base structure
2. ✅ Convert one screen (Welcome) to component pattern
3. ✅ Add Clear widget to all renders
4. ✅ Implement basic mouse support for Welcome screen
5. ✅ Convert remaining screens to components
6. ✅ Add comprehensive mouse support to all screens
7. ✅ UI polish pass on all screens
8. ✅ Test and refine

## Mouse Support Details

### Click Detection
- Track widget areas during render
- Store in component state: `clickable_areas: Vec<(Rect, Action)>`
- On mouse click, check if coordinates match any area
- Execute corresponding action

### Scroll Support
- Mouse wheel up/down on lists
- Scroll preview panels
- Scroll help text

### Drag Support
- Drag scrollbars
- Drag to select (future enhancement)

## UI Style Guide

### Colors
- Primary: Cyan for titles/headers
- Secondary: Yellow for highlights/selection
- Success: Green
- Error: Red
- Info: Blue
- Muted: DarkGray for help text

### Borders
- All panels: Borders::ALL
- Consistent border style across app
- Use rounded corners if supported

### Spacing
- Consistent padding (1-2 chars)
- Clear separation between sections
- Adequate margins

### Typography
- Titles: Bold + Color
- Body: Default style
- Help text: DarkGray
- Errors: Red + Bold

