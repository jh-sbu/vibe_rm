; detection: autotest21's 3000-unit spots were off our cell's floor (nine
; 1024-unit pieces, -1536..1536), and IsDetectedBy was read 2 s after each
; move. Now every spot is on the floor, each read at 2 s and 5 s, and the
; observer stands in the south-west corner facing along the diagonal, so
; the player has room ahead.

; Whether o detects the player at (x, y) from o: "2s/5s".
String Function SeenAt(Actor o, Float x, Float y)
    Player.MoveTo(o, x, y, 0.0, false)
    Utility.Wait(2.0)
    String r = "" + Player.IsDetectedBy(o)
    Utility.Wait(3.0)
    return r + "/" + Player.IsDetectedBy(o)
EndFunction

String Function TestViewCone()
    Actor o = Spawn(NakedBase, 0.0, 0.0)
    o.SetDontMove(true)
    ; The south-west corner, facing north-east along the diagonal.
    o.MoveTo(CellCenter, -1300.0, -1300.0, 0.0, false)
    o.SetAngle(0.0, 0.0, 45.0)
    Utility.Wait(1.0)
    Player.StartSneaking()
    Utility.Wait(1.0)
    String r = "observer facing north-east, the player sneaking (" + Player.IsSneaking() + "), light " + Num(Player.GetLightLevel()) + "; at 300, by bearing from its facing:"
    Int deg = 0
    While deg <= 180
        Float a = 45.0 + deg
        r += " " + deg + ":" + SeenAt(o, 300.0 * Math.sin(a), 300.0 * Math.cos(a))
        deg += 45
    EndWhile
    r += "\n  ahead at 600 " + SeenAt(o, 600.0 * Math.sin(45.0), 600.0 * Math.cos(45.0))
    r += ", 1200 " + SeenAt(o, 1200.0 * Math.sin(45.0), 1200.0 * Math.cos(45.0))
    r += ", 1800 " + SeenAt(o, 1800.0 * Math.sin(45.0), 1800.0 * Math.cos(45.0)) + "\n"
    o.SetActorValue("Blindness", 100.0)
    Utility.Wait(1.0)
    r += "  Blindness 100: ahead at 600 " + SeenAt(o, 600.0 * Math.sin(45.0), 600.0 * Math.cos(45.0)) + ", at 150 " + SeenAt(o, 150.0 * Math.sin(45.0), 150.0 * Math.cos(45.0)) + "\n"
    o.SetActorValue("Blindness", 0.0)
    Gone(o)
    GoCell()
    If Player.IsSneaking()
        Player.StartSneaking()
    EndIf
    return r
EndFunction
