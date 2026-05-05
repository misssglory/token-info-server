# Python example
import socket
import json

def search_token(query, chain=None, limit=10, host='127.0.0.1', port=9002):
    """Search for tokens by name/symbol"""
    request = {
        "type": "search",
        "query": query,
        "limit": limit
    }
    if chain:
        request["chain"] = chain
    
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.connect((host, port))
    
    sock.sendall((json.dumps(request) + "\n").encode())
    response = sock.recv(4096).decode()
    sock.close()
    
    return json.loads(response)

# Examples
if __name__ == "__main__":
    # Search for USDC on any chain
    result = search_token("USDC")
    print(f"Found {result['count']} results:")
    for token in result['results']:
        print(f"  {token['chain']}: {token['label']} -> {token['address']}")
    
    # Search for SOL on Solana only
    result = search_token("SOL", chain="solana")
    print(f"\nSolana results: {result['results']}")
    
    # Search with custom limit
    result = search_token("BTC", limit=5)
    print(f"\nTop 5 BTC-like tokens: {result['count']} found")