# Ain Al Hayat Clinic — Design System Master

> This file is the global source of truth for the application redesign.
> It defines a desktop administrative product, not a marketing website.
> Do not create page-specific design-system files unless they are explicitly requested.

**Project:** Ain Al Hayat Clinic  
**Product:** Arabic-first desktop clinic management application  
**Direction:** RTL by default  
**Reviewed:** 2026-11-07  
**Brand source:** Official clinic logo (`public/clinic-logo.png`)

---

## 1. Design Direction

The application must feel clean, professional, restrained, and dependable during long daily work sessions. It should prioritize fast scanning, predictable navigation, accurate data entry, and clear operational status over decoration or promotional impact.

### Product principles

- Design for receptionists, clinicians, accountants, and administrators completing repeated tasks.
- Prefer information hierarchy, alignment, and spacing over decorative effects.
- Keep frequently used actions visible and predictable.
- Present dense clinical and financial data without making the interface cramped.
- Use progressive disclosure for secondary details and advanced options.
- Preserve context when users move between a table, a record, and a dialog or drawer.
- Use the clinic logo without recoloring, distorting, cropping, or adding effects. Maintain clear space around it.

### This product is not

- A landing page, promotional website, or conversion funnel.
- A hero-led layout with testimonials, pricing blocks, trust badges, or repeated calls to action.
- A card-only interface that replaces suitable tables, forms, or record layouts.
- A visually playful medical theme with gradients, glass effects, floating shapes, or heavy animation.

---

## 2. Token Architecture

Use three layers: primitive values, semantic roles, and component tokens. Components must reference semantic or component tokens rather than raw hex values.

```text
Primitive values → Semantic roles → Component tokens
brand-green-800  → color-primary → button-primary-background
```

### Brand and foundation colors

The solid green palette below is derived from the supplied clinic logo and is the approved visual identity.

| Role | Value | Token | Usage |
|---|---:|---|---|
| Primary | `#006B38` | `--color-primary` | Primary actions, active navigation, key headings, focus ring |
| Secondary | `#008050` | `--color-secondary` | Supporting actions and selected-state emphasis |
| Accent | `#20B040` | `--color-accent` | Limited brand highlight; never normal text on a light surface |
| Background | `#F7FAF8` | `--color-background` | Application workspace and recessed regions |
| Surface | `#FFFFFF` | `--color-surface` | Tables, forms, dialogs, drawers, cards |
| Foreground | `#202B25` | `--color-foreground` | Primary text and icons |
| Muted text | `#68766D` | `--color-muted-foreground` | Secondary text, captions, metadata |
| Border | `#E2EAE4` | `--color-border` | Dividers and nonessential surface separation |

### Accessible derived colors

These values extend the approved palette only where interaction or accessibility requires another state.

| Role | Value | Token | Usage |
|---|---:|---|---|
| On primary | `#FFFFFF` | `--color-on-primary` | Text/icons on primary |
| Primary hover | `#005B30` | `--color-primary-hover` | Pointer hover |
| Primary active | `#004A27` | `--color-primary-active` | Pressed state |
| On secondary | `#FFFFFF` | `--color-on-secondary` | Text/icons on secondary |
| Secondary hover | `#006F45` | `--color-secondary-hover` | Pointer hover |
| Secondary active | `#005D3A` | `--color-secondary-active` | Pressed state |
| On accent | `#202B25` | `--color-on-accent` | Text/icons on accent fills |
| Primary subtle | `#E7F3EC` | `--color-primary-subtle` | Active navigation and quiet selected states |
| Accent subtle | `#EAF8ED` | `--color-accent-subtle` | Noncritical highlight background |
| Strong control border | `#809388` | `--color-control-border` | Inputs and essential control boundaries |
| Focus ring | `#006B38` | `--color-focus` | Keyboard focus indicator |
| Overlay | `rgba(20, 31, 25, 0.48)` | `--color-overlay` | Dialog and drawer scrim |

### Contrast rules

- Primary with white is approximately `6.65:1` and is approved for normal text.
- Secondary with white is approximately `5.00:1` and is approved for normal text.
- Foreground on background is approximately `13.94:1`.
- Muted text on background is approximately `4.54:1`; do not lighten it further for normal-size text.
- Accent on white is approximately `2.86:1`. Do not use accent as normal text, a thin meaningful icon, or a primary button with white text.
- Accent with foreground is approximately `5.12:1`. Use dark foreground when text appears on an accent fill.
- The standard border is intentionally quiet. Use the strong control border where a boundary is necessary to recognize or operate a control.

### Semantic status colors

Brand green must not carry every meaning. Operational status colors require a text label and, where useful, an icon or shape.

| Status | Foreground | Background | Border |
|---|---:|---:|---:|
| Success | `#18733B` | `#E8F5EC` | `#B9DEC6` |
| Information | `#1769AA` | `#EAF3FB` | `#B8D4EB` |
| Warning | `#8A5700` | `#FFF4D6` | `#E8CF8C` |
| Danger | `#B42318` | `#FDECEA` | `#F0BBB5` |

Do not use color alone to distinguish paid/unpaid, active/inactive, completed/cancelled, or positive/negative financial states.

### Theme scope

The current system is light-first. Dark mode is not part of the redesign unless separately scoped and tested; do not generate it by mechanically inverting these colors.

---

## 3. Typography and RTL

### Font family

Use one primary family across Arabic, Latin text, and numerals:

```css
font-family: "IBM Plex Sans Arabic", "Noto Sans Arabic", "Noto Sans", system-ui, sans-serif;
```

IBM Plex Sans Arabic is already available in the repository. Its installed font package includes Arabic, Latin, and extended Latin subsets and supports connected Arabic shaping in modern browsers and the desktop WebView. Load every approved weight that is used rather than relying on synthesized styles.

- Do not use Figtree, Poppins, or another Latin-only heading font for Arabic headings.
- Do not mix a decorative Arabic display face into operational screens.
- Do not apply negative letter spacing to Arabic text. Use normal tracking.
- Use the same family for English labels, codes, and values to maintain consistent rhythm.

### Type scale

| Role | Size | Line height | Weight | Notes |
|---|---:|---:|---:|---|
| Page title | `28px` | `1.35` | `600` | One per screen |
| Section title | `20px` | `1.45` | `600` | Major content group |
| Subsection/title | `16px` | `1.5` | `600` | Card, table, or form group |
| Body | `16px` | `1.6` | `400` | Default Arabic reading text |
| Table/form text | `14–15px` | `1.5` | `400` | Use `15px` when density permits |
| Label | `14px` | `1.45` | `500` | Form and data labels |
| Caption | `13px` | `1.45` | `400` | Secondary metadata only |
| Metric | `24–32px` | `1.25` | `600` | Financial and dashboard values |

Avoid body text below `13px`. Use weight and spacing before introducing additional font sizes.

### RTL and mixed-direction content

- The application root is `dir="rtl"`; components use logical properties such as `padding-inline`, `margin-inline`, and `border-inline-start`.
- Arabic text aligns to inline-start, which is the right side in RTL.
- Phone numbers, record IDs, activation keys, usernames, dates written with slashes, and codes use `dir="ltr"` or `<bdi>` isolation.
- Numeric, date, and currency table columns align consistently to inline-end while the value itself is isolated LTR.
- Use tabular numerals for money, quantities, session counts, timestamps, and identifiers.
- Use one numeral convention throughout operational data: Latin digits `0–9`, formatted consistently with locale-aware grouping and decimals.
- Keep the currency abbreviation attached to its amount and isolate the full value to prevent bidirectional reordering.
- Test Arabic labels, placeholder text, validation messages, truncation, and exported/printed output with real connected text.

---

## 4. Layout, Density, and Elevation

### Spacing scale

Use a compact 4px-based scale suitable for a desktop administrative product.

| Token | Value | Common use |
|---|---:|---|
| `--space-1` | `4px` | Tight internal separation |
| `--space-2` | `8px` | Icon-to-label gap, compact controls |
| `--space-3` | `12px` | Toolbar and field gaps |
| `--space-4` | `16px` | Standard component padding |
| `--space-5` | `20px` | Dense section padding |
| `--space-6` | `24px` | Standard page sections |
| `--space-8` | `32px` | Page gutters and major separation |
| `--space-10` | `40px` | Rare top-level separation |

There is no hero-spacing tier. Large empty areas must not displace useful information below the fold.

### Application shell

- Use a persistent RTL sidebar on the right for top-level navigation.
- Use a compact top bar for page context, search, notifications, account controls, and global utilities.
- Keep the main workspace flexible and suitable for wide tables, with a practical maximum width around `1500–1600px` where needed.
- Standard page padding is `24–32px`; reduce to `16–20px` only when the desktop window is narrow.
- Primary content must remain usable at `1024px` wide and at Windows display scaling of `100%`, `125%`, and `150%`.
- Avoid nested scrolling regions. Tables may have their own horizontal scroll only when necessary.
- Sticky headers and footers must reserve space and must not obscure focused controls or the final table rows.

### Radius

| Token | Value | Usage |
|---|---:|---|
| `--radius-sm` | `6px` | Chips and compact controls |
| `--radius-md` | `8px` | Buttons and fields |
| `--radius-lg` | `10px` | Cards and table containers |
| `--radius-dialog` | `12px` | Dialogs and drawers |
| `--radius-pill` | `999px` | Status indicators only |

Do not apply pill shapes to every button, tab, or input.

### Elevation

| Level | Value | Usage |
|---|---|---|
| Flat | `none` | Cards, tables, forms, page sections |
| Raised | `0 1px 2px rgba(20,31,25,0.08)` | Menus and small popovers |
| Overlay | `0 12px 32px rgba(20,31,25,0.16)` | Dialogs and temporary drawers |

Borders and background contrast establish hierarchy. Do not add a shadow merely because a container is called a card.

---

## 5. Product Patterns

The following are global application patterns, not page-specific design systems.

| Area | Recommended structure |
|---|---|
| Dashboard | Page header → compact operational metrics → pending work/alerts → schedules or recent activity → supporting charts/tables |
| Data management | Page header/actions → search and filters → selection/bulk toolbar when needed → data table → pagination/result count |
| Patient record | Patient identity and critical status → high-priority actions → summary facts → tabbed or sectioned clinical/financial history → audit/context details |
| Forms | Clear title and purpose → logically grouped fields → inline help/errors → summary when needed → stable cancel/save actions |
| Financial management | Balance/totals summary → period and entity filters → ledger/table → reconciliation or payment actions → audit information |
| Subscription management | Status and validity → patient/package facts → used and remaining sessions → transaction/session history → controlled actions |
| Reports | Report selector → date/entity filters → run/export/print actions → summary totals → charts where useful → accessible detail table |
| Administrative settings | Settings navigation → one coherent settings group → explanatory text → fields/permissions → explicit save state |

### Pattern rules

- Each screen has one clearly dominant primary action, not a series of promotional CTAs.
- Prefer tables for comparable records and lists for activity/history; do not force structured datasets into card grids.
- Keep filters visible when they are used repeatedly. Put advanced or rare filters behind disclosure.
- Preserve filter, sort, pagination, and scroll state when returning from a record.
- Use tabs only for peer sections within the same record. Do not use tabs as a substitute for top-level navigation.
- Charts summarize trends or comparisons; they never replace the exact table values needed for clinic or financial work.

---

## 6. Interaction and Motion

### Interaction rules

- Cards and surfaces are noninteractive by default. They have no pointer cursor and no hover effect.
- A clickable card must be an explicit `interactive` variant, use a real link or button semantic, expose an accessible name, and show an affordance such as a chevron or visible action label.
- Apply hover styling only to elements that can be activated. Never use hover as the only way to reveal a required action.
- Do not translate, lift, rotate, or scale cards and buttons on hover.
- Avoid layout-changing pressed states. Use color, border, or subtle state-layer changes.
- Make the whole table row clickable only when every click has the same safe destination. Rows containing selections or multiple actions should keep those controls separate.
- Destructive and financial commitment actions require clear wording and confirmation proportional to risk.

### Motion tokens

| Token | Duration | Use |
|---|---:|---|
| `--duration-fast` | `100ms` | Hover and pressed feedback |
| `--duration-normal` | `150ms` | Focus, selection, disclosure |
| `--duration-overlay` | `180ms` | Dialog/drawer opacity or short slide |

- Transition only the properties that change: color, background-color, border-color, box-shadow, and opacity.
- Never use `transition: all`.
- Motion must communicate a state change. Do not animate content merely as it enters the viewport.
- Do not use parallax, floating decoration, animated gradients, staggered card entrances, or looping interface animation.
- Respect `prefers-reduced-motion`; overlays may appear without movement and loading indicators must remain understandable.

### Focus

- Every operable control has a visible `2px` focus ring using `--color-focus` with a `2px` surface-colored offset.
- Focus indicators must reach at least `3:1` contrast against adjacent colors and must not be clipped.
- Modal dialogs trap focus, return it to the invoking control, and support Escape when dismissal is safe.
- Keyboard order follows the visual RTL reading order without positive `tabindex` values.

---

## 7. Core Components

### 7.1 Navigation

- The right sidebar uses icon plus Arabic label; do not rely on icon-only navigation.
- Active items use primary text, a primary-subtle background, and a stable edge indicator. The state must remain recognizable without color alone.
- Navigation icons use one vector family and a consistent stroke/filled style at each hierarchy level.
- Separate administrative and destructive items from routine clinical navigation.
- Collapsed navigation, if supported, must retain tooltips, accessible names, and a clearly visible active state.
- Use breadcrumbs only when the hierarchy is three or more levels deep.

### 7.2 Cards and surfaces

Default surface:

- Background: `var(--color-surface)`.
- Border: `1px solid var(--color-border)`.
- Radius: `var(--radius-lg)`.
- Shadow: none.
- Padding: `16–24px` according to density.
- Cursor: default.
- Hover: none.

Variants:

| Variant | Use | Interaction |
|---|---|---|
| Surface | Form or information grouping | None |
| Metric | Dashboard value and supporting label | None unless a separate action is shown |
| Interactive | Navigation to one clear destination | Border/background state only; no movement |
| Alert | Operational warning or exception | Actions are explicit buttons/links |

### 7.3 Buttons

| Variant | Appearance | Use |
|---|---|---|
| Primary | Primary background, white text | Save, confirm, create, complete |
| Secondary | Surface background, primary border/text | Supporting action |
| Tertiary/ghost | Transparent, foreground or primary text | Low-emphasis utility |
| Destructive | Danger background, white text | Delete, cancel record, irreversible action |
| Icon | Transparent or quiet surface | Toolbar utility with tooltip and accessible name |

Sizes:

| Size | Height | Horizontal padding | Text |
|---|---:|---:|---:|
| Compact | `36px` | `12px` | `14px` |
| Default | `40px` | `16px` | `14–15px` |
| Prominent | `44px` | `20px` | `16px` |
| Icon | `40 × 40px` minimum | — | — |

State rules:

- Hover darkens the fill or strengthens the border; it does not change position or size.
- Pressed uses the active color and immediate visual feedback.
- Loading preserves button width, exposes `aria-busy`, prevents duplicate submission, and keeps the action label understandable.
- Disabled uses semantic disabled behavior, removes pointer activation, and remains readable. Do not use opacity alone if it makes text illegible.
- Button groups place the primary action first in RTL visual order and keep destructive actions separated.

### 7.4 Forms and controls

- Every field has a persistent visible label. Placeholder text is an example, not a label.
- Default field height is `40–44px`; multiline controls size to content needs.
- Fields use surface background, strong control border, and `--radius-md`.
- Hover strengthens the border slightly. Focus uses the primary border and standard focus ring.
- Error states include an error border, icon where useful, and a specific message directly below the field.
- Validate after blur or submission rather than interrupting Arabic IME composition on every keystroke.
- Required, optional, read-only, disabled, loading, and error states must look and behave differently.
- Group related controls under a heading or fieldset. Use a two-column grid only when fields have a clear relationship and sufficient width.
- Long forms preserve work through draft/autosave where appropriate and warn before discarding unsaved changes.
- Dates, money, percentages, phone numbers, identifiers, and codes use appropriate LTR isolation and input behavior.
- Search fields support clear/reset, a visible label or accessible name, and predictable Enter behavior.

### 7.5 Data tables

Tables are a primary application component, not a fallback.

- Container: surface background, one border, `--radius-lg`, no decorative shadow.
- Header: subtle background, foreground or muted foreground text, `14px/600`, sticky only when it improves long-table scanning.
- Rows: `48px` compact or `56px` standard. Use consistent density within a workflow.
- Cells: `12px 16px` padding, restrained horizontal dividers, no heavy grid on every cell unless required for printed financial reports.
- Text columns align to inline-start. Numeric, date, quantity, and currency columns align to inline-end with tabular LTR values.
- Action columns remain narrow and use labeled actions or an accessible overflow menu.
- Hover is limited to rows that are actually interactive. A noninteractive row must not mimic a link.
- Selected rows use primary-subtle background plus checkbox/state indication.
- Sorting uses a visible direction icon and announces `aria-sort`.
- Keyboard users can reach selection, sorting, pagination, and row actions without entering every noninteractive cell.
- Provide result count, pagination, page size when useful, loading, empty, no-results, and retryable error states.
- Bulk actions appear only after selection and state the number of selected records.
- Long tables may scroll horizontally in a dedicated wrapper; freeze only essential identifier/action columns and avoid excessive sticky columns.
- Large datasets should paginate or virtualize rather than rendering thousands of rows.

### 7.6 Dialogs

- Use dialogs for bounded decisions or short forms, not full navigation flows.
- Widths: `400px` confirmation, `560–640px` standard form, up to `800px` for a justified data-heavy task.
- Maximum height is approximately `80–85vh`; the header and action footer remain visible while the body scrolls.
- Header contains title, concise purpose when needed, and a clear close control.
- Footer contains stable cancel and confirm actions. Destructive confirmation uses explicit action wording.
- Use the overlay color without decorative backdrop blur.
- Trap focus, support safe Escape dismissal, restore focus, and prevent accidental dismissal when unsaved changes exist.

### 7.7 Drawers

- Use a drawer for contextual detail, filters, audit history, or supporting information that benefits from preserving the underlying page.
- Primary navigation remains in the right sidebar; contextual drawers normally open from logical inline-end, which is the left in RTL.
- Typical width is `400–520px`; wider content should become a full page or dialog.
- Drawer content has its own header, close action, logical focus order, and visible action footer when actions are present.
- Avoid stacking multiple drawers or combining a drawer with another modal unless the workflow has no clearer alternative.

### 7.8 Notifications and feedback

| Type | Pattern |
|---|---|
| Field error | Specific inline message associated with the field |
| Form error summary | Focusable summary linking to invalid fields, while retaining inline errors |
| Page alert | Persistent alert near the relevant content with recovery action |
| Toast | Brief confirmation for completed, noncritical actions |
| Banner | Rare application-level outage, license, sync, or security state |

- Toasts never steal focus and use an appropriate live region.
- Success toasts may dismiss automatically after about `4–6s`; errors requiring action remain until dismissed or resolved.
- Messages state what happened and what the user can do next.
- Do not report financial or clinical failure only through a transient toast.

### 7.9 Status indicators

- Use short Arabic labels with a semantic icon or dot when helpful.
- Chips use subtle status backgrounds, dark status text, a `1px` border, and compact padding.
- Do not use the bright accent as a generic success color.
- Distinguish similar states in language: for example active, paused, expired, completed, cancelled, paid, partially paid, and unpaid.
- Do not encode a critical state through color alone; text is mandatory.

### 7.10 Tabs, filters, and pagination

- Tabs use a stable underline or contained selected state. They do not lift, scale, or animate across long distances.
- Filter chips are used only for finite, quickly toggled values. Complex filters belong in fields or a filter drawer.
- Active filters remain visible and individually removable; include a clear-all action when multiple filters can accumulate.
- Pagination preserves filters and sort order and clearly identifies the current range and total.

### 7.11 Charts, reports, and print

- Use line charts for trends, bars for comparisons, and avoid pie/donut charts beyond five categories.
- Provide exact values through tooltips/data labels and an accessible table or summary.
- Do not use red and green as the only comparison pair.
- Keep grid lines quiet, labels readable, and chart animation optional or disabled.
- Report filters and the reporting period must be visible in exported or printed output.
- Printed reports use the official logo, solid primary green, dark text, and clear table rules. They must remain understandable in grayscale.

---

## 8. Accessibility Requirements

- Target WCAG 2.2 AA for application UI.
- Normal text contrast is at least `4.5:1`; large text and meaningful UI boundaries are at least `3:1`.
- All workflows are usable with keyboard only, including tables, dialogs, drawers, menus, date controls, and pagination.
- Focus is visible, logical, and never obscured by sticky UI or overlays.
- Icon-only controls have accessible Arabic names and tooltips where useful.
- Meaning is never communicated by color, icon, position, or hover alone.
- Loading, success, and error changes are announced without moving focus unexpectedly.
- Destructive actions have a safe escape route; recoverable actions should offer undo when practical.
- Authentication must allow paste and password managers.
- Text must remain usable at browser zoom and operating-system display scaling without clipping or loss of actions.
- Respect reduced-motion preferences.
- Test screen-reader order in RTL and verify mixed Arabic/LTR values are announced in a sensible sequence.

---

## 9. Forbidden Patterns

- Cyan, teal, or blue as the primary brand identity.
- Marketing heroes, testimonial rows, pricing sections, logo carousels, promotional badges, or repeated conversion CTAs.
- Automatically clickable cards or pointer cursors on static surfaces.
- Card hover lift, button hover translation, scale-on-press, or decorative transformations.
- `transition: all`, long easing sequences, scroll reveals, parallax, animated gradients, or looping decoration.
- Heavy shadows, glassmorphism, background blur used as decoration, glossy surfaces, or oversized radii.
- Gradients used to replace the solid logo-derived green identity.
- Accent-green text on white or white text on accent green.
- Low-contrast gray text, placeholder-only labels, invisible focus rings, or color-only status.
- Latin-only fonts for Arabic UI, broken Arabic joining, negative Arabic letter spacing, or inconsistent numeral styles.
- Emoji as interface icons or mixed icon families.
- Hiding essential row actions until hover.
- Using cards where a table, form, ledger, timeline, or definition list is structurally clearer.
- Full-width destructive actions placed beside routine actions without separation.
- Raw component hex values when an approved semantic token exists.

---

## 10. Review Checklist

Before application redesign work is accepted, verify:

- [ ] The official logo and approved green palette are used consistently.
- [ ] Primary and secondary actions use accessible foreground pairings; accent is not misused for text.
- [ ] The screen follows a desktop administrative pattern rather than a marketing layout.
- [ ] Static cards have no pointer cursor, hover animation, or implied click behavior.
- [ ] Every interactive element has default, hover, pressed, focus-visible, disabled, and loading states where applicable.
- [ ] No hover or pressed state changes component geometry.
- [ ] Arabic text joins correctly, uses the approved family, and has no artificial letter spacing.
- [ ] RTL layout uses logical properties; phone numbers, IDs, dates, codes, and money remain correctly isolated.
- [ ] Tables support scanning, sorting, pagination, empty/loading/error states, and keyboard operation.
- [ ] Forms use visible labels, grouped fields, inline errors, and safe save/cancel behavior.
- [ ] Dialogs and drawers manage focus and protect unsaved work.
- [ ] Notifications and statuses include clear Arabic text and do not rely on color alone.
- [ ] Financial totals use tabular numerals, explicit currency, and unambiguous positive/negative states.
- [ ] Reports include accessible exact values and remain legible when printed or viewed in grayscale.
- [ ] Reduced motion, zoom, Windows display scaling, and keyboard-only use have been checked.
- [ ] No page-specific design-system file was added without an explicit request.
