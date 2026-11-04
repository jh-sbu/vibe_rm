Scriptname VrmFirstPersonScript extends Quest

GlobalVariable Property Step Auto
Spell Property Probe Auto
Furniture Property Chair Auto
Actor Player
ObjectReference Seat
Int Phase = 0
Int Count = 0
Int Steps = 0
Float Epoch = 0.0
Float Swing = -1.0
Bool Started = false
String ResultSetup
String ResultFirstSword
String ResultThirdSword
String ResultFirstBow
String ResultThirdBow
String ResultFirstMagic
String ResultThirdMagic
String ResultFurniture
String ResultFirstVariables
String ResultThirdVariables
String ResultObservations

Event OnInit()
    RegisterForSingleUpdate(5.0)
EndEvent

Event OnUpdate()
    If !Started
        Started = true
        Player = Game.GetPlayer()
        ResultSetup = "registrations: " + RegisterForAnimationEvent(Player, "weaponSwing")
        ResultSetup += ", " + RegisterForAnimationEvent(Player, "HitFrame")
        ResultSetup += ", " + RegisterForAnimationEvent(Player, "FootLeft")
        ResultSetup += ", " + RegisterForAnimationEvent(Player, "arrowRelease")
        ResultSetup += ", " + RegisterForAnimationEvent(Player, "BeginCastRight")
        ResultSetup += ", " + RegisterForAnimationEvent(Player, "weaponDraw")
        ResultSetup += ", " + RegisterForAnimationEvent(Player, "MRh_SpellFire_Event")
        Debug.MessageBox("First-person probe. Use a throwaway save. Set VrmFirstPersonStep to 1..6 in the console, then close it and draw, walk and perform ONE action. 1 first sword, 2 third sword, 3 first bow, 4 third bow, 5 first spell, 6 third spell. Equip your own items. Each phase records up to 20 event lines. Only the first FootLeft is listed; the rest are counted. Set to 7 for furniture instructions; 8 saves and stops. No animations or attacks are sent by this probe.")
    EndIf
    Int next = Step.GetValueInt()
    If next != Phase
        If Phase >= 1 && Phase <= 6
            AddLine("FootLeft total " + Steps + "\n")
        EndIf
        Phase = next
        Count = 0
        Steps = 0
        If Phase == 1
            ResultFirstSword = ""
        ElseIf Phase == 2
            ResultThirdSword = ""
        ElseIf Phase == 3
            ResultFirstBow = ""
        ElseIf Phase == 4
            ResultThirdBow = ""
        ElseIf Phase == 5
            ResultFirstMagic = ""
        ElseIf Phase == 6
            ResultThirdMagic = ""
        EndIf
        Swing = -1.0
        If Phase == 1 || Phase == 3 || Phase == 5
            Game.ForceFirstPerson()
            ResultFirstVariables = "i1stPerson=" + Player.GetAnimationVariableInt("i1stPerson") + ", IsFirstPerson=" + Player.GetAnimationVariableBool("IsFirstPerson")
        ElseIf Phase == 2 || Phase == 4 || Phase == 6
            Game.ForceThirdPerson()
            ResultThirdVariables = "i1stPerson=" + Player.GetAnimationVariableInt("i1stPerson") + ", IsFirstPerson=" + Player.GetAnimationVariableBool("IsFirstPerson")
        ElseIf Phase == 7
            Seat = Player.PlaceAtMe(Chair, 1, false, false)
            Seat.MoveTo(Player, 0.0, 120.0, 0.0, true)
            ResultFurniture = "outside " + FirstView()
            Debug.MessageBox("A chair with a Furniture Forces 1st Person keyword is beside you. Switch to third person and sit in it manually. While seated, open the console and set VrmFirstPersonStep to 9. Get up, then set VrmFirstPersonStep to 10. This logs IsPC1stPerson before/after ForceFirstPerson in the chair, then after exit.")
        ElseIf Phase == 9
            ReadFurniture()
            Debug.Notification("First-person probe: seated reading taken. Get up, then set VrmFirstPersonStep to 10.")
        ElseIf Phase == 10
            AfterFurniture()
            Debug.Notification("First-person probe: exit reading taken. Set VrmFirstPersonStep to 8 to finish.")
        ElseIf Phase == 8
            ResultObservations = "Please report separately: player shadow in first person; wheel crossing both views; sheathed arms visible; helmet/hood edge visible. Times are real seconds since phase arming, with HitFrame relative to weaponSwing too. Input key timestamps are unavailable without SKSE."
            UnregisterForAnimationEvent(Player, "weaponSwing")
            UnregisterForAnimationEvent(Player, "HitFrame")
            UnregisterForAnimationEvent(Player, "FootLeft")
            UnregisterForAnimationEvent(Player, "arrowRelease")
            UnregisterForAnimationEvent(Player, "BeginCastRight")
            UnregisterForAnimationEvent(Player, "weaponDraw")
            UnregisterForAnimationEvent(Player, "MRh_SpellFire_Event")
            If Seat
                Seat.Disable()
                Seat.Delete()
            EndIf
            Game.RequestSave()
            Debug.MessageBox("Probe saved. Copy the save to tests/results and read vrm-tool test-results tests/results. Also report: first-person shadow, wheel crossing views, sheathed arms, helmet or hood edges. The probe did not replace or remove your equipment.")
            return
        EndIf
        Epoch = Utility.GetCurrentRealTime()
        If Phase >= 1 && Phase <= 6
            Debug.Notification("First-person probe phase " + Phase + " armed: perform one action, then advance Step.")
        EndIf
        Game.RequestSave()
    EndIf
    RegisterForSingleUpdate(0.25)
EndEvent

Event OnAnimationEvent(ObjectReference akSource, String asEventName)
    If Phase < 1 || Phase > 6 || Count >= 20
        return
    EndIf
    If asEventName == "FootLeft"
        ; Walking would otherwise fill the line budget before the action.
        Steps += 1
        If Steps > 1
            return
        EndIf
    EndIf
    Float now = Utility.GetCurrentRealTime()
    If asEventName == "weaponSwing"
        Swing = now
    EndIf
    String line = asEventName + " @" + (now - Epoch)
    If asEventName == "HitFrame" && Swing >= 0.0
        line += " since weaponSwing " + (now - Swing)
    EndIf
    line += "\n"
    AddLine(line)
    Count += 1
EndEvent

Function AddLine(String line)
    If Phase == 1
        ResultFirstSword += line
    ElseIf Phase == 2
        ResultThirdSword += line
    ElseIf Phase == 3
        ResultFirstBow += line
    ElseIf Phase == 4
        ResultThirdBow += line
    ElseIf Phase == 5
        ResultFirstMagic += line
    ElseIf Phase == 6
        ResultThirdMagic += line
    EndIf
EndFunction

String Function FirstView()
    Float before = Player.GetActorValue("OneHanded")
    Player.AddSpell(Probe, false)
    Utility.Wait(2.0)
    Float gain = Player.GetActorValue("OneHanded") - before
    Player.RemoveSpell(Probe)
    Utility.Wait(0.5)
    return "IsPC1stPerson gain=" + gain + " (3=yes, 0=no)"
EndFunction

Function ReadFurniture()
    ResultFurniture += "\nseated sitstate=" + Player.GetSitState() + " " + FirstView()
    Game.ForceFirstPerson()
    Utility.Wait(1.0)
    ResultFurniture += "\nForceFirstPerson sitstate=" + Player.GetSitState() + " " + FirstView()
    Game.RequestSave()
EndFunction

Function AfterFurniture()
    ResultFurniture += "\nafter exit sitstate=" + Player.GetSitState() + " " + FirstView()
    Game.RequestSave()
EndFunction
