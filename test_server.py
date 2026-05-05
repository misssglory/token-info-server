# test_server.py - Make sure this is running
import socket
import json
import sys

def run_test_server():
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)  # Allow reusing address
    server.bind(('127.0.0.1', 1266))
    server.listen(5)
    print("✅ Listening on port 1266...")
    print("Waiting for TG commands from token-info-server...")
    print("-" * 50)
    
    while True:
        try:
            client, addr = server.accept()
            print(f"\n📨 Received connection from {addr}")
            
            data = client.recv(4096).decode()
            if data:
                print(f"Raw: {repr(data)}")
                print(f"Parsed: {data.strip()}")
                
                # Parse and display JSON
                try:
                    cmd = json.loads(data.strip())
                    print(f"\n📋 JSON parsed:")
                    print(json.dumps(cmd, indent=2))
                    
                    # Check if it's a TG command
                    if cmd.get('command') == 'tg':
                        args = cmd.get('args', {})
                        print(f"\n🎮 TG Command Summary:")
                        print(f"  Channel: {args.get('channel')}")
                        print(f"  Timestamp: {args.get('ts')}")
                        print(f"  Mint Address: {args.get('text')}")
                        print(f"  Context: {cmd.get('context')}")
                        print("\n✅ Valid TG command received!")
                    else:
                        print(f"\n⚠️ Unknown command type: {cmd.get('command')}")
                except json.JSONDecodeError as e:
                    print(f"\n❌ Failed to parse JSON: {e}")
                except Exception as e:
                    print(f"\n❌ Error: {e}")
            
            # Send acknowledgment
            client.send(b'{"success":true}\n')
            client.close()
            print("-" * 50)
            
        except KeyboardInterrupt:
            print("\n👋 Shutting down...")
            sys.exit(0)
        except Exception as e:
            print(f"Error: {e}")
            continue

if __name__ == "__main__":
    run_test_server()