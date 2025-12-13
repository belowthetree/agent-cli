#!/usr/bin/env python3
"""
远程指令系统演示脚本

这个脚本演示如何通过 WebSocket 连接到远程服务器并执行指令。
"""

import asyncio
import websockets
import json
import sys

async def test_remote_instruction():
    """测试远程指令系统"""
    uri = "ws://localhost:8080"
    
    try:
        print(f"连接到远程服务器: {uri}")
        async with websockets.connect(uri) as websocket:
            print("连接成功!")
            
            # 1. 首先获取可用的指令列表
            get_commands_request = {
                "request_id": "test_get_commands",
                "input": {"type": "get_commands"}
            }
            
            print("\n1. 获取指令列表...")
            await websocket.send(json.dumps(get_commands_request))
            response = await websocket.recv()
            response_data = json.loads(response)
            
            print(f"响应: {json.dumps(response_data, indent=2, ensure_ascii=False)}")
            
            # 2. 执行清理上下文指令
            clear_context_request = {
                "request_id": "test_clear_context",
                "input": {
                    "type": "instruction",
                    "command": "clear_context",
                    "parameters": {}
                }
            }
            
            print("\n2. 执行清理上下文指令...")
            await websocket.send(json.dumps(clear_context_request))
            response = await websocket.recv()
            response_data = json.loads(response)
            
            print(f"响应: {json.dumps(response_data, indent=2, ensure_ascii=False)}")
            
            # 3. 测试未知指令
            unknown_command_request = {
                "request_id": "test_unknown",
                "input": {
                    "type": "instruction",
                    "command": "unknown_command",
                    "parameters": {}
                }
            }
            
            print("\n3. 测试未知指令...")
            await websocket.send(json.dumps(unknown_command_request))
            response = await websocket.recv()
            response_data = json.loads(response)
            
            print(f"响应: {json.dumps(response_data, indent=2, ensure_ascii=False)}")
            
            print("\n演示完成!")
            
    except ConnectionRefusedError:
        print(f"错误: 无法连接到服务器 {uri}")
        print("请确保远程服务器正在运行:")
        print("  cargo run -- remote start")
        return False
    except Exception as e:
        print(f"错误: {e}")
        return False
    
    return True

async def main():
    """主函数"""
    print("=" * 60)
    print("远程指令系统演示")
    print("=" * 60)
    
    success = await test_remote_instruction()
    
    if success:
        print("\n✅ 所有测试通过!")
    else:
        print("\n❌ 测试失败")
        sys.exit(1)

if __name__ == "__main__":
    asyncio.run(main())
