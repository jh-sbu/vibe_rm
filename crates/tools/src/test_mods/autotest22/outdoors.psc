; Outdoors, from where the run began: the sun's position (autotest21 read
; 0, 0, 0 in our interior cell). Skipped when the run began indoors.

String Function TestSunPosition()
    Player.MoveTo(StartMarker, 0.0, 0.0, 0.0, true)
    Utility.Wait(5.0)
    If Player.IsInInterior()
        return "skipped: the run began indoors\n"
    EndIf
    Float hour = GameHour.GetValue()
    String r = "outdoors; GetSunPositionX/Y/Z by game hour:"
    Float[] hours = new Float[4]
    hours[0] = 6.0
    hours[1] = 9.0
    hours[2] = 12.0
    hours[3] = 18.0
    Int i = 0
    While i < 4
        GameHour.SetValue(hours[i])
        Utility.Wait(1.0)
        r += " " + hours[i] + ": " + Num(Game.GetSunPositionX()) + ", " + Num(Game.GetSunPositionY()) + ", " + Num(Game.GetSunPositionZ()) + ";"
        i += 1
    EndWhile
    GameHour.SetValue(hour)
    return r + " (the player at " + Num(Player.GetPositionX()) + ", " + Num(Player.GetPositionY()) + ", " + Num(Player.GetPositionZ()) + ")\n"
EndFunction
