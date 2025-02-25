  Core Architecture

  The project is organized as a Rust workspace with three main components:

  1. Server: A Bevy-powered game server that manages the grid world, bot connections, and game state.
  2. Simple-bot: A basic client implementation that demonstrates how to connect to the server and send commands.
  3. Swarm-lib: A shared library containing the communication protocol and common data structures.

  Technical Implementation

  Server

  The server uses the Bevy game engine with its Entity Component System (ECS) to manage the game state. Key components include:

  - Gridworld (server/src/gridworld.rs): Implements a 2D grid system with A* pathfinding.
  - Bot Handler (server/src/bot_handler.rs): Manages TCP connections with bot clients and routes messages.
  - Tilemap (server/src/tilemap.rs): Renders the game world using Bevy ECS Tilemap.

  The server provides visual feedback with a rendered grid showing the current game state, using ASCII-based assets (server/assets/ascii.png).

  Communication Protocol

  The communication layer in swarm-lib/src/protocol.rs is particularly interesting:

  - Implements a custom wire protocol with version checking
  - Uses TCP sockets with a structured message format
  - Serializes data with bincode for efficient transmission
  - Includes message types for actions (movement), queries (radar), and responses
  - Handles connection management with retry mechanisms

  Bot Implementation

  The simple-bot provides a command-line interface for manual control, demonstrating the bot API:

  - Connects to the server and receives a bot ID
  - Can send movement commands (up, down, left, right)
  - Can query the server for information (position, radar)
  - Uses a multi-threaded approach with channels for communication

  Project Direction & Spirit

  The project appears to be aiming toward a programmable bot competition framework, similar to games like Screeps or RoboCode, but with a simpler 2D grid-based approach. The separation of components suggests a design where:

  1. Players would write their own bot implementations using the swarm-lib API
  2. These bots would compete in a common server environment
  3. The game would focus on strategy and algorithms rather than real-time control

  The name "Swarm-Skirmish" hints at the potential for swarm-based tactics where multiple bots could work together as teams. The current implementation with radar functionality and team designations (Player and Enemy)
  suggests a capture-the-flag or territory-control game mechanic might be planned.

  The project is early but thoughtfully structured - the modular design allows for expansion of the game mechanics while maintaining a clean separation between server, protocol, and client implementations. The use of Rust
  ensures safety and performance, while Bevy provides a solid foundation for game development.