; Back to the test cell, and what the trip left behind.

String Function TestLeaveWarehouse()
    Player.ModActorValue("Health", -1000.0)
    GoCell()
    return "back in the test cell\n"
EndFunction

; loose-objects: the bowl TestRememberBefore knocked, back in the cell.
String Function TestRememberAfter()
    Utility.Wait(1.0)
    return "the knocked bowl after the cell unloaded and loaded: x " + Num(RememberBowl.GetPositionX() - CellCenter.GetPositionX()) + ", y " + Num(RememberBowl.GetPositionY() - CellCenter.GetPositionY()) + ", z " + Num(RememberBowl.GetPositionZ() - CellCenter.GetPositionZ()) + "\n"
EndFunction
