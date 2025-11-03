#!/bin/bash

# TDD Orchestrator Setup Script
# This script sets up the TDD testing environment

set -e

echo "╔═══════════════════════════════════════════════════════════════════╗"
echo "║              TDD Orchestrator Setup                               ║"
echo "╚═══════════════════════════════════════════════════════════════════╝"
echo ""

# Check if we're in the web directory
if [ ! -f "package.json" ]; then
    echo "❌ Error: Please run this script from the web/ directory"
    exit 1
fi

echo "📦 Installing dependencies..."
yarn install

echo ""
echo "🎭 Installing Playwright browsers..."
yarn tdd-install

echo ""
echo "📁 Creating report directories..."
mkdir -p test-orchestrator/reports/screenshots
mkdir -p test-orchestrator/reports/videos

echo ""
echo "✅ Setup complete!"
echo ""
echo "╔═══════════════════════════════════════════════════════════════════╗"
echo "║                    Next Steps                                     ║"
echo "╚═══════════════════════════════════════════════════════════════════╝"
echo ""
echo "1. Start your development server:"
echo "   yarn start"
echo ""
echo "2. In another terminal, run a test workflow:"
echo "   yarn tdd-test dt-integration"
echo ""
echo "3. View available workflows:"
echo "   yarn tdd-test --help"
echo ""
echo "4. Read the documentation:"
echo "   cat test-orchestrator/README.md"
echo ""
echo "Happy TDD testing! 🚀"
echo ""
