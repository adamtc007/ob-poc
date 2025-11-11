#!/bin/bash

# OB-POC Web Interface Setup Script
# This script sets up the complete web interface for the agentic CRUD system

set -e

echo "🚀 OB-POC Web Interface Setup"
echo "=============================="

# Check if we're in the right directory
if [ ! -f "../rust/Cargo.toml" ]; then
    echo "❌ Error: Please run this script from the web-interface directory"
    echo "   Expected location: ob-poc/web-interface/"
    exit 1
fi

echo "📂 Current directory: $(pwd)"
echo "✅ Located in correct directory"

# Check for required tools
echo ""
echo "🔍 Checking prerequisites..."

# Check Node.js
if command -v node &> /dev/null; then
    NODE_VERSION=$(node --version)
    echo "✅ Node.js: $NODE_VERSION"
else
    echo "❌ Node.js not found. Please install Node.js 18+ from https://nodejs.org"
    exit 1
fi

# Check npm
if command -v npm &> /dev/null; then
    NPM_VERSION=$(npm --version)
    echo "✅ npm: $NPM_VERSION"
else
    echo "❌ npm not found. Please install npm"
    exit 1
fi

# Check Rust (for API server)
if command -v cargo &> /dev/null; then
    RUST_VERSION=$(rustc --version)
    echo "✅ Rust: $RUST_VERSION"
else
    echo "❌ Rust not found. Please install Rust from https://rustup.rs"
    exit 1
fi

# Check PostgreSQL
if command -v psql &> /dev/null; then
    PSQL_VERSION=$(psql --version | head -1)
    echo "✅ PostgreSQL: $PSQL_VERSION"
else
    echo "⚠️  PostgreSQL not found. Install PostgreSQL for full functionality"
fi

echo ""
echo "🏗️  Setting up project structure..."

# Create directory structure
mkdir -p frontend
mkdir -p api
mkdir -p docs
mkdir -p scripts
mkdir -p config

echo "✅ Created directory structure"

# Frontend setup with Next.js
echo ""
echo "⚛️  Setting up React/Next.js frontend..."

cd frontend

# Check if package.json already exists
if [ -f "package.json" ]; then
    echo "📦 Frontend already initialized, updating dependencies..."
    npm install
else
    echo "📦 Initializing new Next.js project..."

    # Create Next.js app
    npx create-next-app@latest . --typescript --tailwind --eslint --app --src-dir --import-alias "@/*"

    # Install additional dependencies
    echo "📦 Installing additional dependencies..."
    npm install @tanstack/react-query axios lucide-react @radix-ui/react-dialog @radix-ui/react-select @radix-ui/react-tabs class-variance-authority clsx tailwind-merge

    # Install dev dependencies
    npm install -D @types/node
fi

cd ..

# API setup
echo ""
echo "🔧 Setting up Rust API server..."

cd api

# Create Cargo.toml for API server
cat > Cargo.toml << EOF
[package]
name = "ob-poc-api"
version = "0.1.0"
edition = "2021"

[dependencies]
# Web framework
axum = { version = "0.7", features = ["macros"] }
tower = "0.4"
tower-cors = "0.3"
tower-http = { version = "0.5", features = ["fs", "trace", "cors"] }

# Async runtime
tokio = { version = "1", features = ["full"] }

# HTTP client
reqwest = { version = "0.11", features = ["json"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Database
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid"] }

# Utilities
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Environment
dotenvy = "0.15"

# Reference to main ob-poc library
ob-poc = { path = "../../rust" }
EOF

# Create basic API structure
mkdir -p src/handlers src/models src/middleware

cd ..

# Configuration files
echo ""
echo "⚙️  Creating configuration files..."

# Environment template
cat > .env.example << EOF
# Database Configuration
DATABASE_URL=postgresql://user:password@localhost/ob_poc_db

# AI Service Configuration
OPENAI_API_KEY=your_openai_api_key_here
GEMINI_API_KEY=your_gemini_api_key_here

# API Server Configuration
API_PORT=3001
API_HOST=0.0.0.0
CORS_ORIGINS=http://localhost:3000

# Frontend Configuration
NEXT_PUBLIC_API_URL=http://localhost:3001

# Security
JWT_SECRET=your_jwt_secret_here
BCRYPT_ROUNDS=12

# Logging
RUST_LOG=debug
EOF

# Docker Compose for development
cat > docker-compose.dev.yml << EOF
version: '3.8'

services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_DB: ob_poc_db
      POSTGRES_USER: ob_poc_user
      POSTGRES_PASSWORD: ob_poc_password
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ../sql:/docker-entrypoint-initdb.d:ro
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ob_poc_user -d ob_poc_db"]
      interval: 10s
      timeout: 5s
      retries: 5

  api:
    build:
      context: .
      dockerfile: api/Dockerfile
    ports:
      - "3001:3001"
    environment:
      - DATABASE_URL=postgresql://ob_poc_user:ob_poc_password@postgres/ob_poc_db
      - RUST_LOG=debug
    depends_on:
      postgres:
        condition: service_healthy
    volumes:
      - ./api:/app
      - ../rust:/ob-poc
    working_dir: /app

  frontend:
    build:
      context: .
      dockerfile: frontend/Dockerfile.dev
    ports:
      - "3000:3000"
    environment:
      - NEXT_PUBLIC_API_URL=http://localhost:3001
    volumes:
      - ./frontend:/app
      - /app/node_modules
    working_dir: /app
    command: npm run dev

volumes:
  postgres_data:
EOF

# Development scripts
echo ""
echo "📜 Creating development scripts..."

# Development start script
cat > scripts/dev-start.sh << 'EOF'
#!/bin/bash

echo "🚀 Starting OB-POC Development Environment"
echo "=========================================="

# Check if .env exists
if [ ! -f .env ]; then
    if [ -f .env.example ]; then
        echo "📋 Creating .env from .env.example..."
        cp .env.example .env
        echo "⚠️  Please edit .env with your actual configuration values"
    else
        echo "❌ No .env.example found"
        exit 1
    fi
fi

# Start services
echo "🐳 Starting Docker services..."
docker-compose -f docker-compose.dev.yml up -d postgres

echo "⏳ Waiting for database to be ready..."
sleep 5

# Apply database migrations
echo "🗄️  Applying database migrations..."
cd ../rust
cargo run --bin apply_migrations --features="database"
cd ../web-interface

echo "🔧 Starting API server..."
cd api
cargo run &
API_PID=$!
cd ..

echo "⚛️  Starting frontend..."
cd frontend
npm run dev &
FRONTEND_PID=$!
cd ..

echo ""
echo "✅ Development environment started!"
echo "   Frontend: http://localhost:3000"
echo "   API: http://localhost:3001"
echo "   Database: localhost:5432"
echo ""
echo "Press Ctrl+C to stop all services"

# Wait for interrupt
trap "echo '🛑 Stopping services...'; kill $API_PID $FRONTEND_PID; docker-compose -f docker-compose.dev.yml down; exit" INT
wait
EOF

chmod +x scripts/dev-start.sh

# Build script
cat > scripts/build.sh << 'EOF'
#!/bin/bash

echo "🏗️  Building OB-POC Web Interface"
echo "================================="

# Build frontend
echo "⚛️  Building frontend..."
cd frontend
npm run build
cd ..

# Build API
echo "🔧 Building API..."
cd api
cargo build --release
cd ..

echo "✅ Build completed!"
echo "   Frontend build: frontend/.next/"
echo "   API binary: api/target/release/ob-poc-api"
EOF

chmod +x scripts/build.sh

# Database setup script
cat > scripts/setup-database.sh << 'EOF'
#!/bin/bash

echo "🗄️  Setting up OB-POC Database"
echo "=============================="

# Check for DATABASE_URL
if [ -z "$DATABASE_URL" ]; then
    if [ -f .env ]; then
        source .env
    fi
fi

if [ -z "$DATABASE_URL" ]; then
    echo "❌ DATABASE_URL not set. Please configure your database connection."
    echo "   Example: export DATABASE_URL='postgresql://user:password@localhost/ob_poc_db'"
    exit 1
fi

echo "📡 Database URL: $(echo $DATABASE_URL | sed 's/:\/\/.*:.*@/:\/\/***:***@/')"

# Apply migrations
echo "🔄 Applying database migrations..."

cd ../sql

echo "   📋 Applying base schema..."
psql "$DATABASE_URL" -f 00_init_schema.sql

echo "   📋 Applying dictionary attributes..."
psql "$DATABASE_URL" -f 03_seed_dictionary_attributes.sql

echo "   📋 Applying agentic CRUD schema..."
psql "$DATABASE_URL" -f 14_agentic_crud_phase1_schema.sql

cd ../web-interface

echo "✅ Database setup completed!"
EOF

chmod +x scripts/setup-database.sh

# Documentation
echo ""
echo "📚 Creating documentation..."

cat > docs/README.md << EOF
# OB-POC Web Interface

This is the web interface for the OB-POC (Onboarding Proof of Concept) agentic CRUD system.

## Architecture

### Frontend (Next.js + TypeScript + Tailwind)
- **Framework**: Next.js 14 with App Router
- **Language**: TypeScript
- **Styling**: Tailwind CSS
- **Components**: Radix UI primitives
- **State Management**: TanStack Query (React Query)
- **HTTP Client**: Axios

### Backend (Rust + Axum)
- **Framework**: Axum web framework
- **Database**: PostgreSQL with SQLx
- **Authentication**: JWT-based
- **AI Integration**: OpenAI/Gemini APIs
- **Core Logic**: ob-poc Rust library

### Database (PostgreSQL)
- **Schema**: ob-poc namespace
- **Migrations**: SQL-based migrations
- **Features**: Entity management, CRUD operations, audit trails

## Quick Start

1. **Prerequisites**
   - Node.js 18+
   - Rust 1.70+
   - PostgreSQL 15+
   - Docker (optional, for development)

2. **Setup**
   \`\`\`bash
   ./setup.sh
   \`\`\`

3. **Configure Environment**
   \`\`\`bash
   cp .env.example .env
   # Edit .env with your configuration
   \`\`\`

4. **Setup Database**
   \`\`\`bash
   ./scripts/setup-database.sh
   \`\`\`

5. **Start Development**
   \`\`\`bash
   ./scripts/dev-start.sh
   \`\`\`

## Features

### Implemented
- ✅ Entity CRUD operations
- ✅ AI-powered DSL generation
- ✅ Transaction management
- ✅ Audit logging
- ✅ Multi-provider AI support

### Planned
- 🔄 Web-based entity creation
- 🔄 Real-time operation monitoring
- 🔄 User authentication
- 🔄 Role-based access control
- 🔄 Dashboard and analytics

## API Endpoints

### Entity Management
- \`POST /api/entities\` - Create entity
- \`GET /api/entities\` - Search entities
- \`PUT /api/entities/:id\` - Update entity
- \`DELETE /api/entities/:id\` - Delete entity

### AI Operations
- \`POST /api/ai/generate-dsl\` - Generate DSL from natural language
- \`POST /api/ai/validate-dsl\` - Validate DSL syntax

### Transaction Management
- \`POST /api/transactions\` - Create batch transaction
- \`GET /api/transactions/:id\` - Get transaction status
- \`POST /api/transactions/:id/rollback\` - Rollback transaction

## Development

### Frontend Development
\`\`\`bash
cd frontend
npm run dev      # Start development server
npm run build    # Build for production
npm run lint     # Run ESLint
npm run test     # Run tests
\`\`\`

### Backend Development
\`\`\`bash
cd api
cargo run        # Start development server
cargo build      # Build for production
cargo test       # Run tests
cargo clippy     # Run linter
\`\`\`

### Database Management
\`\`\`bash
./scripts/setup-database.sh      # Initialize database
./scripts/migrate.sh             # Apply new migrations
./scripts/seed-data.sh           # Add sample data
\`\`\`

## Deployment

### Docker Deployment
\`\`\`bash
docker-compose up -d
\`\`\`

### Manual Deployment
1. Build all components: \`./scripts/build.sh\`
2. Configure production environment
3. Deploy frontend to CDN/static hosting
4. Deploy API server to cloud provider
5. Configure PostgreSQL database

## Contributing

1. Follow the existing code style
2. Write tests for new features
3. Update documentation
4. Submit pull requests for review

## License

MIT License - Internal POC development
EOF

# Main README for web interface
cat > README.md << EOF
# OB-POC Web Interface

Production-ready web interface for the agentic CRUD entity management system.

## Quick Start

Run the setup script to get started:

\`\`\`bash
./setup.sh
\`\`\`

This will:
- Check prerequisites (Node.js, Rust, PostgreSQL)
- Set up project structure
- Initialize frontend and backend
- Create configuration files
- Generate development scripts

## Next Steps

After setup completion:

1. **Configure environment**: Edit \`.env\` file with your settings
2. **Setup database**: Run \`./scripts/setup-database.sh\`
3. **Start development**: Run \`./scripts/dev-start.sh\`

## Documentation

See \`docs/README.md\` for comprehensive documentation.

## Architecture

- **Frontend**: Next.js + TypeScript + Tailwind CSS
- **Backend**: Rust + Axum + PostgreSQL
- **AI**: OpenAI/Gemini integration
- **Features**: Entity CRUD, AI DSL generation, transaction management
EOF

echo "✅ Created configuration files"
echo "✅ Created development scripts"
echo "✅ Created documentation"

echo ""
echo "🎉 Web Interface Setup Complete!"
echo "================================="
echo ""
echo "📁 Project Structure:"
echo "   web-interface/"
echo "   ├── frontend/          # Next.js React application"
echo "   ├── api/               # Rust API server"
echo "   ├── scripts/           # Development and deployment scripts"
echo "   ├── config/            # Configuration files"
echo "   ├── docs/              # Documentation"
echo "   └── .env.example       # Environment template"
echo ""
echo "📋 Next Steps:"
echo "   1. Configure your environment:"
echo "      cp .env.example .env"
echo "      # Edit .env with your actual values"
echo ""
echo "   2. Setup database:"
echo "      ./scripts/setup-database.sh"
echo ""
echo "   3. Start development:"
echo "      ./scripts/dev-start.sh"
echo ""
echo "   4. Visit http://localhost:3000"
echo ""
echo "📚 Documentation: See docs/README.md for detailed information"
echo ""
echo "🚀 Ready for full-stack agentic CRUD development!"
