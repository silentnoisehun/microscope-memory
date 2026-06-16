"""
Microscope Memory — LangChain Integration Example (v1 API)
(v0.7.0 Public Beta)

This example demonstrates how to connect a LangChain agent to the 
Microscope Memory engine using the Spine Bridge REST API.
"""

import requests
import json
from typing import Optional, List
from langchain.tools import tool
from langchain_openai import ChatOpenAI
from langchain.agents import AgentExecutor, create_openai_functions_agent
from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder

# 1. Configuration
BRIDGE_URL = "http://localhost:6060/v1"

# 2. Define Custom Tools for Microscope Memory
@tool
def recall_memory(query: str, k: int = 5) -> str:
    """
    Search into the high-speed hierarchical memory of the system.
    Returns the most relevant past experiences or facts related to the query.
    """
    try:
        response = requests.get(f"{BRIDGE_URL}/recall", params={"q": query, "k": k})
        if response.status_code == 200:
            memories = response.json()
            if not memories:
                return "No relevant memories found."
            
            formatted = "\n".join([
                f"- [{m['layer']} D{m['depth']}] {m['text']}" 
                for m in memories
            ])
            return f"Relevant memories:\n{formatted}"
        else:
            return f"Error connecting to memory engine: {response.text}"
    except Exception as e:
        return f"Memory engine connection failed: {str(e)}"

@tool
def store_memory(text: str, layer: str = "long_term", importance: int = 5) -> str:
    """
    Store new information into the system's long-term memory.
    Use this for facts, user preferences, or important conversation context.
    """
    try:
        payload = {"text": text, "layer": layer, "importance": importance}
        response = requests.post(f"{BRIDGE_URL}/remember", json=payload)
        if response.status_code == 200:
            return "Information successfully committed to long-term memory."
        else:
            return f"Failed to store memory: {response.text}"
    except Exception as e:
        return f"Memory engine connection failed: {str(e)}"

# 3. Setup Agent
def run_agent_demo():
    print("--- Microscope Memory + LangChain Agent Demo ---")
    
    # Check if engine is running
    try:
        health = requests.get(f"{BRIDGE_URL}/status").json()
        print(f"Status: Engine Active (v{health['version']}), {health['blocks']} blocks.")
    except:
        print("Error: Microscope Memory Spine Bridge is not running at", BRIDGE_URL)
        print("Start it with: microscope-mem bridge")
        return

    llm = ChatOpenAI(model="gpt-4-turbo", temperature=0)
    tools = [recall_memory, store_memory]
    
    prompt = ChatPromptTemplate.from_messages([
        ("system", "You are an AI assistant with access to a high-speed hierarchical memory system. "
                   "If you learn something new and important, store it. "
                   "If you need context, recall it."),
        MessagesPlaceholder(variable_name="chat_history"),
        ("human", "{input}"),
        MessagesPlaceholder(variable_name="agent_scratchpad"),
    ])

    agent = create_openai_functions_agent(llm, tools, prompt)
    agent_executor = AgentExecutor(agent=agent, tools=tools, verbose=True)

    # Demo 1: Storing information
    print("\n> Storing user preference...")
    agent_executor.invoke({"input": "My favorite programming language is Rust, because of the memory safety.", "chat_history": []})

    # Demo 2: Recalling information
    print("\n> Recalling user preference in a new session...")
    agent_executor.invoke({"input": "What is my favorite programming language and why?", "chat_history": []})

if __name__ == "__main__":
    # Note: Requires langchain, langchain-openai, requests
    # run_agent_demo()
    print("Example script loaded. Ensure Microscope Memory bridge is running.")
    print("Usage: python examples/langchain_integration.py")
