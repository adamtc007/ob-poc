# API Keys Setup - OB-POC Project

This document explains how to set up API keys for the OB-POC AI examples using macOS Keychain integration.

## ✅ Current Setup Status

Your API keys are now configured to load automatically from macOS Keychain:

- **OpenAI API Key**: ✅ Configured in keychain
- **Gemini API Key**: ✅ Configured in keychain  
- **Shell Profile**: ✅ Permanent exports added to `~/.zshrc`
- **Convenient Script**: ✅ `run-with-keys.sh` created
- **Helpful Aliases**: ✅ Added to shell profile

## 🔑 How It Works

### Keychain Integration
Your API keys are securely stored in macOS Keychain and automatically loaded using:

```bash
export OPENAI_API_KEY="$(security find-generic-password -w -s "OPENAI_API_KEY")"
export GEMINI_API_KEY="$(security find-generic-password -w -s "GEMINI_API_KEY")"
```

### Shell Profile Setup
Added to your `~/.zshrc` file:
```bash
# OB-POC AI API Keys from macOS Keychain
export OPENAI_API_KEY="$(security find-generic-password -w -s "OPENAI_API_KEY")"
export GEMINI_API_KEY="$(security find-generic-password -w -s "GEMINI_API_KEY")"

# OB-POC Convenient Aliases
alias ob-demo="cd /path/to/ob-poc && ./run-with-keys.sh"
alias ob-test="cd /path/to/ob-poc && ./run-with-keys.sh test"
alias ob-mock="cd /path/to/ob-poc && ./run-with-keys.sh mock_openai_demo"
alias ob-ai="cd /path/to/ob-poc && ./run-with-keys.sh ai_dsl_onboarding_demo"
alias ob-parse="cd /path/to/ob-poc && ./run-with-keys.sh parse_zenith"
```

## 🚀 Usage

### Method 1: Using the Convenient Script
```bash
# Test API key setup
./run-with-keys.sh test

# Run AI examples
./run-with-keys.sh ai_dsl_onboarding_demo
./run-with-keys.sh simple_openai_dsl_demo
./run-with-keys.sh mock_openai_demo
./run-with-keys.sh parse_zenith

# List all available examples
./run-with-keys.sh list

# Show help
./run-with-keys.sh
```

### Method 2: Using Shell Aliases (after `source ~/.zshrc`)
```bash
ob-test          # Test API keys
ob-ai            # Full AI workflow demo
ob-mock          # Mock demo (no API keys needed)
ob-parse         # DSL parsing demo
ob-demo          # Show script help
```

### Method 3: Direct Cargo Commands (after shell reload)
```bash
cargo run --example test_api_keys
cargo run --example ai_dsl_onboarding_demo
cargo run --example simple_openai_dsl_demo
cargo run --example mock_openai_demo
cargo run --example parse_zenith
```

## 📋 Available Examples

| Example | Description | API Keys Required |
|---------|-------------|-------------------|
| `test_api_keys` | Verify API key setup | ✅ |
| `ai_dsl_onboarding_demo` | Full AI workflow demo | ✅ |
| `simple_openai_dsl_demo` | Basic OpenAI integration | ✅ OpenAI only |
| `mock_openai_demo` | Architecture demo | ❌ None |
| `parse_zenith` | DSL parsing with UBO case study | ❌ None |
| `minimal_orchestration_demo` | Core DSL orchestration | ❌ None |

## 🔧 Troubleshooting

### API Keys Not Found
If you get "API key not found" errors:

1. **Check keychain entries**:
   ```bash
   security find-generic-password -s "OPENAI_API_KEY"
   security find-generic-password -s "GEMINI_API_KEY"
   ```

2. **Add keys to keychain** (if missing):
   ```bash
   security add-generic-password -s "OPENAI_API_KEY" -a "$USER" -w "your-openai-key"
   security add-generic-password -s "GEMINI_API_KEY" -a "$USER" -w "your-gemini-key"
   ```

3. **Reload shell configuration**:
   ```bash
   source ~/.zshrc
   ```

### Getting API Keys

#### OpenAI API Key
1. Go to: https://platform.openai.com/api-keys
2. Sign in with your OpenAI account
3. Click "Create new secret key"
4. Copy the key (starts with `sk-`)

#### Google Gemini API Key  
1. Go to: https://makersuite.google.com/app/apikey
2. Or try: https://aistudio.google.com/
3. Sign in with your Google account
4. Create an API key
5. Copy the key (usually starts with `AIzaSy`)

## 🎯 Quick Test

To verify everything is working:

```bash
# Test 1: Check keychain access
security find-generic-password -w -s "OPENAI_API_KEY" | wc -c
security find-generic-password -w -s "GEMINI_API_KEY" | wc -c

# Test 2: Run API key test
./run-with-keys.sh test

# Test 3: Run mock demo (no API keys needed)
./run-with-keys.sh mock_openai_demo
```

Expected output for working setup:
```
🔑 OB-POC Example Runner
Loading API keys from macOS Keychain...
✅ OpenAI API key loaded
✅ Gemini API key loaded
🎉 All API key tests passed!
```

## 🔐 Security Benefits

- **Secure Storage**: API keys stored in macOS Keychain, not in plain text files
- **No Git Exposure**: Keys never accidentally committed to version control
- **Access Control**: Keychain access protected by macOS security
- **Easy Rotation**: Update keys in keychain without changing code

## 📝 Notes

- The shell exports are added to `~/.zshrc` and will be available in new terminal sessions
- The `run-with-keys.sh` script provides a convenient wrapper for all examples
- Mock examples work without any API keys for testing the architecture
- Database features require additional setup (see project documentation)

---

**Status**: ✅ **API Keys Configured and Working**  
**Last Updated**: 2025-11-13  
**Setup Method**: macOS Keychain + Shell Profile Integration