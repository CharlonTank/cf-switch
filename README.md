# cf-switch

A Cloudflare profile switcher for `flarectl`. Easily switch between multiple Cloudflare accounts and manage common tasks like cache purging.

## Installation

```bash
# Build from source
cargo build --release
cp target/release/cf-switch /usr/local/bin/

# Install flarectl (required)
brew install cloudflare/cloudflare/flarectl
```

## Shell Setup

Add this to your shell config for the `cfs` alias:

**Fish** (`~/.config/fish/config.fish`):
```fish
function cfs
    switch $argv[1]
        case use ''
            cf-switch $argv | source
        case '*'
            cf-switch $argv
    end
end
```

**Bash/Zsh** (`~/.bashrc` or `~/.zshrc`):
```bash
cfs() { eval "$(cf-switch "$@")"; }
```

## Usage

```bash
# Toggle between profiles
cfs

# Switch to specific profile
cfs use myprofile

# List all profiles
cfs list

# Show current profile
cfs current

# Purge cache (uses profile's default zone)
cfs purge

# Purge specific zone
cfs purge example.com

# Add Lamdera DNS record
cfs add-lamdera-app
cfs add-lamdera-app myapp.com
```

## Adding a Profile

```bash
cf-switch add <name> -e <email> -t <token> -z <zone>

# Example
cf-switch add mysite -e me@example.com -t "abc123..." -z example.com
```

## Creating a Cloudflare API Token

1. Go to [Cloudflare Dashboard > API Tokens](https://dash.cloudflare.com/profile/api-tokens)

2. Click **Create Token**

3. Click **Get started** (Custom token)

4. Configure the token:
   - **Token name**: `flarectl` (or any descriptive name)
   - **Permissions**:
     - For cache purge: `Zone > Cache Purge > Purge`
     - For DNS management: `Zone > DNS > Edit`
   - **Zone Resources**: `Include > All zones` (or specific zone)

5. Click **Continue to summary** → **Create Token**

6. **Copy the token immediately** (you won't see it again!)

7. Add to cf-switch:
   ```bash
   cf-switch add myprofile -e your@email.com -t "your-token-here" -z yourdomain.com
   ```

## Token Permissions Reference

| Task | Permission Required |
|------|---------------------|
| `cfs purge` | Zone > Cache Purge > Purge |
| `cfs add-lamdera-app` | Zone > DNS > Edit |

## Config Location

Profiles are stored in `~/.cf-switch.json`

Active credentials are written to `~/.cloudflare.env`
