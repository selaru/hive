#!/bin/bash

# Exit immediately if any command in a pipe fails, or if a variable is unset
set -eo pipefail

# Print banner
echo "========================================="
echo "        Starting Hive Locally"
echo "========================================="

# 1. Load environment variables and apply self-healing local overrides for Docker hostnames
if [ -f .env ]; then
  echo "Loading and self-healing .env variables..."
  while IFS= read -r line || [ -n "$line" ]; do
    # Skip comments and empty lines
    if [[ "$line" =~ ^[[:space:]]*# ]] || [[ -z "$line" ]]; then
      continue
    fi
    # Export the variable
    export "$line"
  done < .env

  # Heal Docker network hostnames to localhost/127.0.0.1 equivalents for local host run
  if [[ "$DATABASE_URL" == *"postgres:5432"* || "$DATABASE_URL" == *"@postgres/"* || "$DATABASE_URL" == *"//postgres:"* ]]; then
    DATABASE_URL="postgres://hive:hive@127.0.0.1:5432/hive"
    export DATABASE_URL
  fi
  if [[ "$REDIS_URL" == *"redis:6379"* || "$REDIS_URL" == *"//redis:"* ]]; then
    REDIS_URL="redis://127.0.0.1:6379"
    export REDIS_URL
  fi
  if [[ "$EXECUTOR_URL" == *"executor:3001"* || "$EXECUTOR_URL" == *"//executor:"* ]]; then
    EXECUTOR_URL="http://127.0.0.1:3001"
    export EXECUTOR_URL
  fi
  if [[ "$CONTAINER_PROJECTS_DIR" == "/app/projects" ]]; then
    CONTAINER_PROJECTS_DIR="../projects"
    export CONTAINER_PROJECTS_DIR
  fi
else
  echo "Error: .env file not found. Copy .env.example to .env first."
  exit 1
fi

# 2. Check local dependencies
echo "Checking and starting database and cache services..."

# Check and Start Postgres
if ! nc -z 127.0.0.1 5432; then
  echo "PostgreSQL is not running on port 5432. Starting postgres container..."
  docker compose up -d postgres
  echo "Waiting for PostgreSQL to be ready..."
  until nc -z 127.0.0.1 5432; do
    sleep 1
  done
  echo "PostgreSQL is ready!"
fi

# Check and Start Redis
if ! nc -z 127.0.0.1 6379; then
  echo "Redis is not running on port 6379. Starting redis container..."
  docker compose up -d redis
  echo "Waiting for Redis to be ready..."
  until nc -z 127.0.0.1 6379; do
    sleep 1
  done
  echo "Redis is ready!"
fi

# 3. Clean up any existing logs
rm -f local_backend.log local_executor.log local_frontend.log

# 4. Define cleanup behavior on exit (Ctrl+C)
cleanup() {
  echo -e "\nStopping all services..."
  if [ -n "$BACKEND_PID" ]; then kill $BACKEND_PID 2>/dev/null || true; fi
  if [ -n "$EXECUTOR_PID" ]; then kill $EXECUTOR_PID 2>/dev/null || true; fi
  if [ -n "$FRONTEND_PID" ]; then kill $FRONTEND_PID 2>/dev/null || true; fi
  echo "Cleanup complete. Goodbye!"
  exit 0
}
trap cleanup SIGINT SIGTERM

# 5. Start Backend API
echo "Starting Backend API (logging to local_backend.log)..."
cd backend
cargo run --release --bin hive-api > ../local_backend.log 2>&1 &
BACKEND_PID=$!
cd ..

# 6. Start Executor
echo "Starting Sandboxed Executor (logging to local_executor.log)..."
cd backend
cargo run --release --bin hive-executor > ../local_executor.log 2>&1 &
EXECUTOR_PID=$!
cd ..

# 7. Start Frontend
echo "Starting Frontend Dev Server (logging to local_frontend.log)..."
cd frontend
npm run dev > ../local_frontend.log 2>&1 &
FRONTEND_PID=$!
cd ..

# 8. Wait for frontend server to initialize
echo "Waiting for services to spin up..."
sleep 3

# 9. Open browser window
URL="http://localhost:5173"
echo "Opening browser to $URL ..."

if [[ "$OSTYPE" == "darwin"* ]]; then
  open "$URL"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
  if grep -q Microsoft /proc/version 2>/dev/null; then
    powershell.exe Start "$URL" # WSL
  else
    xdg-open "$URL"
  fi
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" ]]; then
  start "$URL"
else
  echo "Please open $URL manually."
fi

echo "========================================="
echo "Hive is running!"
echo "Press Ctrl+C to stop all services."
echo "========================================="

# Keep script running to monitor background processes
while true; do
  # If any process died, trigger exit
  if ! kill -0 $BACKEND_PID 2>/dev/null; then
    echo "Error: Backend API stopped unexpectedly. Check local_backend.log for details."
    cleanup
  fi
  if ! kill -0 $EXECUTOR_PID 2>/dev/null; then
    echo "Error: Executor service stopped unexpectedly. Check local_executor.log for details."
    cleanup
  fi
  if ! kill -0 $FRONTEND_PID 2>/dev/null; then
    echo "Error: Frontend server stopped unexpectedly. Check local_frontend.log for details."
    cleanup
  fi
  sleep 1
done
