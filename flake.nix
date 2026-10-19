{
  description = "Token Info Server with PostgreSQL caching and OpenSSL support";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    (flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        
        # Development dependencies
        devDeps = with pkgs; [
          # OpenSSL and related libraries
          openssl
          pkg-config
          
          # PostgreSQL client libraries and tools
          postgresql
          postgresql.lib
          
          # Build tools
          gcc
          gnumake
          cmake
          
          # Additional utilities
          curl
          git
          cargo
          rustc
          rustfmt
          clippy
          
          # Database migration tools
          sqlx-cli
          
          # Network tools
          lsof
        ];
        
        # Environment variables for development
        devEnvVars = {
          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          DATABASE_URL = "postgresql://postgres:postgres@localhost:5432/token_cache";
          RUST_BACKTRACE = "1";
          RUST_LOG = "debug";
        };
        
        # Shell script to setup development environment
        setupScript = pkgs.writeShellScriptBin "setup-dev" ''
          #!/usr/bin/env bash
          echo "🔧 Setting up development environment for Token Info Server"
          
          # Create PostgreSQL database if not exists
          if pg_isready -h localhost -p 5432 > /dev/null 2>&1; then
            echo "✅ PostgreSQL is running"
            
            # Check if database exists
            if psql -lqt | cut -d \| -f 1 | grep -qw token_cache; then
              echo "✅ Database 'token_cache' already exists"
            else
              echo "📦 Creating database 'token_cache'..."
              createdb token_cache
              echo "✅ Database created"
            fi
            
            # Run migrations
            if command -v sqlx > /dev/null; then
              echo "🔄 Running database migrations..."
              sqlx migrate run
              echo "✅ Migrations completed"
            else
              echo "⚠️  sqlx-cli not found, skipping migrations"
            fi
          else
            echo "⚠️  PostgreSQL is not running. Start it with: sudo systemctl start postgresql"
            echo "   Then run this script again"
          fi
          
          # Create .env file if it doesn't exist
          if [ ! -f .env ]; then
            echo "📝 Creating .env file from template..."
            cat > .env << 'EOF'
# RPC Endpoints
SOLANA_RPC=https://api.mainnet-beta.solana.com
ETHEREUM_RPC=https://eth-mainnet.g.alchemy.com/v2/demo
BSC_RPC=https://bsc-dataseed.binance.org

# Database Configuration
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/token_cache
DB_HOST=localhost
DB_PORT=5432
DB_NAME=token_cache
DB_USER=postgres
DB_PASSWORD=postgres
DB_MAX_CONNECTIONS=10
CACHE_TTL_SECONDS=86400

# TCP Endpoint Configuration
TCP_ENDPOINT_ENABLED=true
TCP_ENDPOINT_HOST=127.0.0.1
TCP_ENDPOINT_PORT=9003
TCP_FORWARD_TOKENS=true
EOF
            echo "✅ .env file created"
          fi
          
          echo "✨ Development environment ready!"
          echo ""
          echo "Available commands:"
          echo "  cargo build          - Build the project"
          echo "  cargo run            - Run the server"
          echo "  cargo test           - Run tests"
          echo "  cargo watch -x run   - Auto-reload on changes"
          echo "  sqlx migrate run     - Run database migrations"
        '';
        
        # Shell script to start development services
        startServices = pkgs.writeShellScriptBin "start-services" ''
          #!/usr/bin/env bash
          echo "🚀 Starting development services..."
          
          # Start PostgreSQL if not running
          if ! pg_isready -h localhost -p 5432 > /dev/null 2>&1; then
            echo "Starting PostgreSQL..."
            sudo systemctl start postgresql
            sleep 2
          fi
          
          # Check if TCP endpoint is running (assuming it's the executor server)
          if nc -z localhost 9003 2>/dev/null; then
            echo "✅ TCP endpoint is running on port 9003"
          else
            echo "⚠️  TCP endpoint not running on port 9003"
            echo "   Start your executor server first"
          fi
          
          echo "✅ Services ready"
        '';
        
        # Shell script to run database migrations
        runMigrations = pkgs.writeShellScriptBin "run-migrations" ''
          #!/usr/bin/env bash
          echo "🔄 Running database migrations..."
          
          export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/token_cache"
          
          if command -v sqlx > /dev/null; then
            sqlx migrate run
            echo "✅ Migrations completed"
          else
            echo "❌ sqlx-cli not found. Install with: cargo install sqlx-cli"
            exit 1
          fi
        '';
        
      in
      {
        # Development shell
        devShells.default = pkgs.mkShell {
          buildInputs = devDeps ++ [ setupScript startServices runMigrations ];
          
          shellHook = ''
            echo "🐚 Entering Token Info Server development shell"
            echo "================================================"
            echo "📦 Nix packages loaded:"
            echo "   - OpenSSL ${pkgs.openssl.version}"
            echo "   - PostgreSQL ${pkgs.postgresql.version}"
            echo "   - Rust: $(rustc --version)"
            echo "   - Cargo: $(cargo --version)"
            echo ""
            echo "🔧 Environment variables set:"
            echo "   - OPENSSL_DIR=${devEnvVars.OPENSSL_DIR}"
            echo "   - DATABASE_URL=${devEnvVars.DATABASE_URL}"
            echo ""
            echo "💡 Quick start:"
            echo "   1. Run 'start-services' to start PostgreSQL"
            echo "   2. Run 'setup-dev' to setup the database"
            echo "   3. Run 'cargo run' to start the server"
            echo ""
            echo "📚 Available commands:"
            echo "   - setup-dev        - Setup development environment"
            echo "   - start-services   - Start PostgreSQL and check services"
            echo "   - run-migrations   - Run database migrations"
            echo "   - cargo build        - Build the project"
            echo "   - cargo test         - Run tests"
            echo "   - cargo doc --open   - Generate and open documentation"
            echo "================================================"
          '';
          
          # Set environment variables
          OPENSSL_DIR = devEnvVars.OPENSSL_DIR;
          OPENSSL_LIB_DIR = devEnvVars.OPENSSL_LIB_DIR;
          OPENSSL_INCLUDE_DIR = devEnvVars.OPENSSL_INCLUDE_DIR;
          PKG_CONFIG_PATH = devEnvVars.PKG_CONFIG_PATH;
          DATABASE_URL = devEnvVars.DATABASE_URL;
          RUST_BACKTRACE = devEnvVars.RUST_BACKTRACE;
          RUST_LOG = devEnvVars.RUST_LOG;
        };
        
        # Development shell with additional tools
        devShells.dev = pkgs.mkShell {
          buildInputs = devDeps ++ [ setupScript startServices runMigrations ] ++ (with pkgs; [
            # Additional development tools
            cargo-audit
            cargo-deny
            cargo-outdated
            cargo-tarpaulin
            bacon
            hyperfine
            jq
            httpie
          ]);
          
          shellHook = ''
            echo "🐚 Entering Token Info Server development shell (full tools)"
            echo "============================================================"
            echo "✅ Additional tools available:"
            echo "   - cargo-audit     - Security vulnerability checker"
            echo "   - cargo-deny      - License and dependency checker"
            echo "   - cargo-tarpaulin - Code coverage tool"
            echo "   - bacon           - Background code checker"
            echo "   - httpie          - HTTP client for testing"
            echo "   - jq              - JSON processor"
            echo ""
            echo "🎯 Run 'bacon' for automatic background checking"
            echo "============================================================"
          '';
          
          OPENSSL_DIR = devEnvVars.OPENSSL_DIR;
          OPENSSL_LIB_DIR = devEnvVars.OPENSSL_LIB_DIR;
          OPENSSL_INCLUDE_DIR = devEnvVars.OPENSSL_INCLUDE_DIR;
          PKG_CONFIG_PATH = devEnvVars.PKG_CONFIG_PATH;
          DATABASE_URL = devEnvVars.DATABASE_URL;
          RUST_BACKTRACE = devEnvVars.RUST_BACKTRACE;
          RUST_LOG = devEnvVars.RUST_LOG;
        };
        
        # Production build environment
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "token-info-server";
          version = "0.1.0";
          
          src = ./.;
          
          # cargoLock = {
          #   lockFile = ./Cargo.lock;
          #   allowBuiltinFetchGit = true;
          # };
          cargoHash = "sha256-2XZCe2auh3jYigF4rQaE5iw/HowOo8uNNuAJ8PX1F1I="; # Replace with actual got: hash after Rustls feature change
          
          nativeBuildInputs = with pkgs; [
            pkg-config
            openssl
            postgresql
          ];
          
          buildInputs = with pkgs; [
            openssl
            postgresql.lib
          ];
          
          OPENSSL_DIR = devEnvVars.OPENSSL_DIR;
          OPENSSL_LIB_DIR = devEnvVars.OPENSSL_LIB_DIR;
          OPENSSL_INCLUDE_DIR = devEnvVars.OPENSSL_INCLUDE_DIR;
          
          # TLS certificates are available through the systemd environment.
          # RUST_LOG = "info";
          
          meta = with pkgs.lib; {
            description = "Token Info Server with PostgreSQL caching";
            homepage = "https://github.com/yourusername/token-info-server";
            license = licenses.mit;
            mainProgram = "token-info-server";
          };
        };
        
        # Docker container configuration
        packages.docker = pkgs.dockerTools.buildLayeredImage {
          name = "token-info-server";
          tag = "latest";
          
          contents = [
            pkgs.bashInteractive
            pkgs.coreutils
            pkgs.cacert
            pkgs.openssl
          ];
          
          config = {
            Cmd = [ "${self.packages.${system}.default}/bin/token-info-server" ];
            Env = [
              "RUST_LOG=info"
              "OPENSSL_DIR=${pkgs.openssl}"
              "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            ];
            ExposedPorts = {
              "9002/tcp" = {};
              "9003/tcp" = {};
            };
          };
        };
        
        # Formatter for nix files
        formatter = pkgs.nixfmt-rfc-style;
        
        # Check for common issues
        checks = {
          clippy = pkgs.runCommand "clippy-check" {
            buildInputs = [ pkgs.cargo pkgs.clippy pkgs.rustc ];
          } ''
            cargo clippy --manifest-path ${./.}/Cargo.toml -- -D warnings
            touch $out
          '';
          
          fmt = pkgs.runCommand "fmt-check" {
            buildInputs = [ pkgs.cargo pkgs.rustfmt ];
          } ''
            cargo fmt --manifest-path ${./.}/Cargo.toml -- --check
            touch $out
          '';
        };
      }
    ));
}
