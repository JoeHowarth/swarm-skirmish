Action Ideas:
- MoveTo
- MoveAdjacent
- MoveToExactPath (cancels if path blocked instead of automatically pathfinding around it)
- Harvest (resources)
- Pickup (items)
- Transfer (to pawn, to ground)
- Build
- Interact (use charger, use rapair station, etc.)

Messaging
- SendMsg
- SendBroadcast
- SendRadarData

Current System
--------------

ActionRx -drained-> ActionQueue -popped-into-> InProgress
InProgress -composed-of-> ComputedActionQueue 
    Computed popped until empty or status change


Proposed System
---------------
BotResp(s) -> ActionQueue --> ActionResults -> BotUpdate

ActionQueue holds 
ActionContainer { kind, id, state, status }

handle_incoming_bot_resp
- cancels actions specified
- pushes new actions to queue

progress_action_queues(mut world)
- 