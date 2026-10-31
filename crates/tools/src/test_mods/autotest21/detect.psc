; Detection, in the test cell: an observer held facing a way, the player
; moved round it, sneaking or not.

; Whether the observer detects the player at (x, y) from it, after 2 s.
Bool Function SeenAt(Actor o, Float x, Float y)
    Player.MoveTo(o, x, y, 0.0, false)
    Utility.Wait(2.0)
    return Player.IsDetectedBy(o)
EndFunction

String Function TestViewCone()
    Actor o = Spawn(NakedBase, 0.0, 0.0)
    o.SetDontMove(true)
    o.MoveTo(CellCenter, 0.0, 0.0, 0.0, false)
    o.SetAngle(0.0, 0.0, 0.0)
    o.SetActorValue("Sneak", 100.0)
    Utility.Wait(1.0)
    Player.StartSneaking()
    Utility.Wait(1.0)
    String r = "observer (Sneak 100) facing north, the player sneaking (" + Player.IsSneaking() + ") 400 off, at bearings:"
    Int deg = 0
    While deg <= 180
        Float x = 400.0 * Math.sin(deg)
        Float y = 400.0 * Math.cos(deg)
        r += " " + deg + ":" + SeenAt(o, x, y)
        deg += 30
    EndWhile
    r += "\n  at 1500 ahead " + SeenAt(o, 0.0, 1500.0) + ", 3000 ahead " + SeenAt(o, 0.0, 3000.0) + ", 1500 behind " + SeenAt(o, 0.0, -1500.0) + "\n"
    o.SetActorValue("Blindness", 100.0)
    Utility.Wait(1.0)
    r += "  Blindness 100: 400 ahead " + SeenAt(o, 0.0, 400.0) + ", 150 ahead " + SeenAt(o, 0.0, 150.0)
    o.SetActorValue("Blindness", 0.0)
    r += "\n  player's light level here " + Num(Player.GetLightLevel()) + "\n"
    Player.StartSneaking()
    Utility.Wait(0.5)
    r += "  not sneaking (" + Player.IsSneaking() + "): 3000 behind " + SeenAt(o, 0.0, -3000.0) + ", 1500 behind " + SeenAt(o, 0.0, -1500.0) + "\n"
    Gone(o)
    GoCell()
    return r
EndFunction

String Function TestAlertFlag()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    a.SetDontMove(true)
    Utility.Wait(1.0)
    a.SetAlert(true)
    Utility.Wait(1.0)
    String r = "SetAlert(true): IsAlerted " + a.IsAlerted() + ", combat state " + a.GetCombatState() + ", drawn " + a.IsWeaponDrawn()
    a.SetAlert(false)
    Utility.Wait(1.0)
    r += "; SetAlert(false): " + a.IsAlerted() + ", state " + a.GetCombatState() + "\n"
    a.AddItem(IronSword, 1, true)
    a.EquipItem(IronSword, false, true)
    Utility.Wait(0.5)
    a.DrawWeapon()
    Float t0 = Now()
    Float again = -1.0
    Bool redrawn = false
    While a.IsWeaponDrawn() || Now() - t0 < 0.5
        If !redrawn && Now() - t0 >= 1.0
            a.DrawWeapon()
            redrawn = true
            again = Now() - t0
        EndIf
        If Now() - t0 > 15.0
            r += "(still drawn at 15 s) "
            a.SetAlert(false)
        EndIf
        Utility.Wait(0.1)
    EndWhile
    r += "unalerted, DrawWeapon, again at " + Num(again) + " s: sheathed " + Num(Now() - t0) + " s after the first\n"
    Gone(a)
    return r
EndFunction

; detection.md: the action sound settings the plugins lack.
String Function TestSoundLevels()
    String r = "iSoundLevelSilent " + Game.GetGameSettingInt("iSoundLevelSilent") + ", Normal " + Game.GetGameSettingInt("iSoundLevelNormal") + ", Loud " + Game.GetGameSettingInt("iSoundLevelLoud") + ", VeryLoud " + Game.GetGameSettingInt("iSoundLevelVeryLoud") + "\n"
    r += "fCombatDetectionFleeingLostRemoveTime " + Game.GetGameSettingFloat("fCombatDetectionFleeingLostRemoveTime") + ", fCombatLowFleeingTargetHitPercent " + Game.GetGameSettingFloat("fCombatLowFleeingTargetHitPercent")
    r += ", fCombatDetectionLostRemoveTime " + Game.GetGameSettingFloat("fCombatDetectionLostRemoveTime") + ", fCombatDialogueFleeMinElapsedTime " + Game.GetGameSettingFloat("fCombatDialogueFleeMinElapsedTime") + ", Max " + Game.GetGameSettingFloat("fCombatDialogueFleeMaxElapsedTime") + "\n"
    r += "iFriendHitCombatAllowed " + Game.GetGameSettingInt("iFriendHitCombatAllowed") + ", fMagicWardPowerMaxBase " + Game.GetGameSettingFloat("fMagicWardPowerMaxBase") + ", iAICombatRestoreHealthPercentage " + Game.GetGameSettingInt("iAICombatRestoreHealthPercentage") + "\n"
    return r
EndFunction
