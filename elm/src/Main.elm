module Main exposing (main)

import Browser exposing (..)
import Browser.Navigation as Nav
import Date
import Html exposing (..)
import Html.Attributes exposing (..)
import Html.Events exposing (..)
import Http
import Json.Decode as Decode exposing (Decoder)
import Json.Decode.Pipeline as JsonP
import Json.Encode as Encode
import List
import List.Extra as LExtra
import RemoteData exposing (WebData)
import Url
import Url.Parser as Parser exposing ((</>))
import Url.Parser.Query as Query


type ExpandCollapse
    = Expand
    | Collapse


subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.none


main : Program Flags Model Msg
main =
    Browser.document
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        }


type alias Flags =
    { rust_version : String
    , current_date : String
    }


type alias Model =
    { flags : Flags
    , installer_options : WebData InstallerOption
    , hide_list : List String
    , locked_options : List String
    , select_list : List String
    , selected : String
    }


type Msg
    = NoOp
    | ReceiveStructure (WebData InstallerOption)
    | ToggleExpand ExpandCollapse String
    | SelectDocs String
    | AddCheckOption ( String, String )
    | AddCheckOptionWithChildren ( String, String, List OptionNode )
    | AddRadioOption ( String, String )
    | RemoveOption String
    | RemoveOptionWithChildren ( String, List OptionNode )


init : Flags -> ( Model, Cmd Msg )
init flags =
    ( { flags = flags
      , installer_options = RemoteData.Loading
      , locked_options = []
      , hide_list = []
      , select_list = []
      , selected = ""
      }
    , fetchStructure
    )


type RadioCheck
    = Radio
    | RadioChecked
    | RadioFolder
    | Checked
    | Unchecked
    | Locked
    | ParentLocked


fetchStructure : Cmd Msg
fetchStructure =
    Http.get
        { url = "/structure"
        , expect = Http.expectJson (RemoteData.fromResult >> ReceiveStructure) decodeInstallerOption
        }


radioCheckFromString : Decoder RadioCheck
radioCheckFromString =
    Decode.string
        |> Decode.andThen
            (\s ->
                case s of
                    "Radio" ->
                        Decode.succeed Radio

                    "RadioChecked" ->
                        Decode.succeed RadioChecked

                    "RadioFolder" ->
                        Decode.succeed RadioFolder

                    "Checked" ->
                        Decode.succeed Checked

                    "Locked" ->
                        Decode.succeed Locked

                    "ParentLocked" ->
                        Decode.succeed ParentLocked

                    "Unchecked" ->
                        Decode.succeed Unchecked

                    _ ->
                        Decode.fail <| "Unknown RadioCheck Option"
            )


radioCheckToString : RadioCheck -> String
radioCheckToString rc =
    case rc of
        Radio ->
            "Radio"

        RadioChecked ->
            "RadioChecked"

        RadioFolder ->
            "RadioFolder"

        Checked ->
            "Checked"

        Locked ->
            "Locked"

        Unchecked ->
            "Unchecked"

        ParentLocked ->
            "ParentLocked"


type alias OptionNode =
    { name : String
    , radio_check : RadioCheck
    , children : InstallerOption
    , depth : Int
    , parent : String
    }


type InstallerOption
    = InstallerOption (List OptionNode)


defInstallerOption : InstallerOption
defInstallerOption =
    InstallerOption []


decodeOptionNode : Decoder OptionNode
decodeOptionNode =
    Decode.succeed OptionNode
        |> JsonP.required "name" Decode.string
        |> JsonP.required "radio_check" radioCheckFromString
        |> JsonP.optional "children" (Decode.lazy (\_ -> decodeInstallerOption)) (InstallerOption [])
        |> JsonP.required "depth" Decode.int
        |> JsonP.required "parent" Decode.string


decodeInstallerOption : Decoder InstallerOption
decodeInstallerOption =
    Decode.map InstallerOption <| Decode.list (Decode.lazy (\_ -> decodeOptionNode))


update : Msg -> Model -> ( Model, Cmd Msg )
update message model =
    case message of
        NoOp ->
            ( model, Cmd.none )

        ToggleExpand expand_collapse id ->
            case expand_collapse of
                Expand ->
                    ( { model | hide_list = LExtra.remove id model.hide_list }, Cmd.none )

                Collapse ->
                    ( { model | hide_list = id :: model.hide_list }, Cmd.none )

        SelectDocs id ->
            ( { model | selected = id }, Cmd.none )

        AddCheckOption ( parent, name ) ->
            ( { model | select_list = (parent ++ "/" ++ name) :: model.select_list |> LExtra.unique }, Cmd.none )

        AddCheckOptionWithChildren ( parent, name, children ) ->
            let
                new_select_list =
                    (parent ++ "/" ++ name)
                        :: model.select_list
                        |> addOptionsRecursively children
                        |> LExtra.unique
            in
            ( { model | select_list = new_select_list }, Cmd.none )

        AddRadioOption ( parent, name ) ->
            let
                closest =
                    getClosestParent parent

                remove_list =
                    List.filter
                        (\s -> getClosestParentFromId s == closest)
                        model.select_list

                new_select_list =
                    List.map (\s -> LExtra.remove s model.select_list) remove_list |> List.concat
            in
            ( { model | select_list = (parent ++ "/" ++ name) :: new_select_list |> LExtra.unique }, Cmd.none )

        RemoveOption id ->
            ( { model | select_list = LExtra.remove id model.select_list |> LExtra.unique }, Cmd.none )

        RemoveOptionWithChildren ( id, children ) ->
            let
                new_select_list =
                    removeOptionsRecursively children model.select_list
                        |> LExtra.remove id
                        |> LExtra.unique
            in
            ( { model | select_list = new_select_list }, Cmd.none )

        ReceiveStructure res ->
            case res of
                RemoteData.Success r ->
                    let
                        hide_list =
                            getParents r

                        select_list =
                            getSelected r

                        locked_options =
                            getLocked r
                    in
                    ( { model | installer_options = res, hide_list = hide_list, select_list = select_list, locked_options = locked_options }, Cmd.none )

                _ ->
                    ( { model | installer_options = res }, Cmd.none )


addOptionsRecursively : List OptionNode -> List String -> List String
addOptionsRecursively options current =
    List.append current
        (List.map (\i -> i.parent ++ "/" ++ i.name) options
            ++ (List.map (\c -> getParents c.children) options |> List.concat)
        )


removeOptionsRecursively : List OptionNode -> List String -> List String
removeOptionsRecursively options current =
    let
        to_remove =
            List.map (\i -> i.parent ++ "/" ++ i.name) options
                ++ (List.map (\c -> getParents c.children) options |> List.concat)
    in
    List.filter (\c -> not <| List.member c to_remove) current


getClosestParent : String -> String
getClosestParent s =
    String.split "/" s |> List.reverse |> List.head |> Maybe.withDefault ""


getClosestParentFromId : String -> String
getClosestParentFromId s =
    String.split "/" s |> List.take (List.length (String.split "/" s) - 1) |> List.reverse |> List.head |> Maybe.withDefault ""


getParents : InstallerOption -> List String
getParents option =
    LExtra.unique
        (List.map .parent (unwrapInstallerOption option) |> List.filter (\item -> List.length (String.split "/" item) > 1))
        ++ (List.map (\c -> getParents c.children) (unwrapInstallerOption option) |> List.concat)


getSelected : InstallerOption -> List String
getSelected option =
    LExtra.unique <|
        (List.map (\i -> i.parent ++ "/" ++ i.name) <| List.filter (\i -> i.radio_check == Checked || i.radio_check == RadioChecked) (unwrapInstallerOption option))
            ++ (List.concat <| List.map (\l -> getSelected l.children) (unwrapInstallerOption option))


getLocked : InstallerOption -> List String
getLocked option =
    LExtra.unique <|
        (List.map (\i -> i.parent ++ "/" ++ i.name) <| List.filter (\i -> i.radio_check == Locked) (unwrapInstallerOption option))
            ++ (List.concat <| List.map (\l -> getLocked l.children) (unwrapInstallerOption option))


view : Model -> Browser.Document Msg
view model =
    let
        innerHTML =
            displayInstaller model

        version =
            model.flags.rust_version
    in
    { title = "Network Addon Mod Installer v" ++ version
    , body =
        [ innerHTML ]
    }


displayInstaller : Model -> Html Msg
displayInstaller model =
    div [ class "application-area" ]
        [ div [ style "padding" "15px" ]
            [ h3 [ class "title is-3" ] [ text <| "Network Addon Mod Installer v" ++ model.flags.rust_version ]
            , br [] []
            , div [ class "columns" ]
                [ case model.installer_options of
                    RemoteData.Success opts ->
                        div [ class "is-half", style "max-height" "90vh", style "overflow-y" "auto", style "min-height" "90vh", style "width" "50vw" ]
                            (List.concat <|
                                List.map (displayOptions model) <|
                                    unwrapInstallerOption opts
                            )

                    RemoteData.Loading ->
                        div [] [ text "Loading..." ]

                    RemoteData.NotAsked ->
                        div [] [ text "Not Asked" ]

                    RemoteData.Failure f ->
                        let
                            err =
                                case f of
                                    Http.NetworkError ->
                                        "Network Error"

                                    Http.BadBody bb ->
                                        "Bad Body: " ++ bb

                                    Http.BadStatus bs ->
                                        "Bad Status: " ++ String.fromInt bs

                                    Http.BadUrl bu ->
                                        "Bad Url: " ++ bu

                                    Http.Timeout ->
                                        "Timeout"
                        in
                        div [] [ text err ]
                , div [ class "is-half", style "max-height" "90vh", style "overflow-y" "auto", style "min-height" "90vh", style "width" "50vw" ] <| List.map (\s -> tr [] [ text s ]) <| List.sort <| model.locked_options ++ model.select_list
                ]
            ]
        ]


displayOptions : Model -> OptionNode -> List (Html Msg)
displayOptions model option =
    let
        tabs =
            String.concat <|
                List.map (\_ -> "       ") <|
                    List.range 0 option.depth

        name_ =
            option.name
    in
    [ div
        [ style "display" <|
            if List.member option.parent model.hide_list then
                "none"

            else
                "inherit"
        ]
        [ tr
            []
            [ td [ style "white-space" "pre", style "padding-right" "10px" ]
                [ text tabs ]
            , td [ style "white-space" "pre", style "padding-right" "20px" ] <|
                case option.children of
                    InstallerOption list ->
                        if List.length list > 0 then
                            if List.member (option.parent ++ "/" ++ option.name) model.hide_list then
                                [ button [ class "button is-small is-outlined", onClick (ToggleExpand Expand (option.parent ++ "/" ++ option.name)) ] [ text "＋" ] ]

                            else
                                [ button [ class "button is-small is-outlined", onClick (ToggleExpand Collapse (option.parent ++ "/" ++ option.name)) ] [ text "－" ] ]

                        else
                            [ button [ class "button is-small is-outlined invisible no-text" ] [ text "＋" ] ]
            , case ( option.radio_check, List.member (option.parent ++ "/" ++ option.name) model.select_list ) of
                ( Radio, False ) ->
                    radioUnchecked option

                ( Radio, True ) ->
                    radioChecked option

                ( RadioChecked, False ) ->
                    radioUnchecked option

                ( RadioChecked, True ) ->
                    radioChecked option

                ( RadioFolder, _ ) ->
                    radioFolder option

                ( Checked, False ) ->
                    uncheckedBox model option

                ( Checked, True ) ->
                    checkedBox option

                ( Unchecked, False ) ->
                    uncheckedBox model option

                ( Unchecked, True ) ->
                    checkedBox option

                ( Locked, _ ) ->
                    locked option

                ( ParentLocked, _ ) ->
                    parentLocked model option
            , td [ style "vertical-align" "middle", style "padding-left" "20px" ]
                [ button
                    [ class "button is-small"
                    , onClick (SelectDocs (option.parent ++ "/" ++ option.name))
                    , name option.parent
                    , class
                        (if model.selected == (option.parent ++ "/" ++ option.name) then
                            "is-black"

                         else
                            "invisible"
                        )
                    ]
                    [ text <| name_ ]
                ]
            ]
        , br [] []
        , div [] <|
            List.concat <|
                case option.children of
                    InstallerOption list ->
                        List.map (displayOptions model) list
        ]
    ]


radioUnchecked : OptionNode -> Html Msg
radioUnchecked option =
    td [ style "vertical-align" "middle" ]
        [ input
            [ style "transform" "scale(1.5)"
            , type_ "radio"
            , name <| String.fromInt option.depth ++ "_" ++ option.parent
            , onClick (AddRadioOption ( option.parent, option.name ))
            ]
            []
        ]


radioChecked : OptionNode -> Html Msg
radioChecked option =
    td [ style "vertical-align" "middle" ]
        [ input [ style "transform" "scale(1.5)", type_ "radio", attribute "checked" "", name <| String.fromInt option.depth ++ "_" ++ option.parent ] [] ]


radioFolder : OptionNode -> Html Msg
radioFolder _ =
    td [ style "vertical-align" "middle" ]
        [ input [ style "transform" "scale(1.5)", type_ "radio", disabled True ] [] ]


checkedBox : OptionNode -> Html Msg
checkedBox option =
    let
        children =
            unwrapInstallerOption option.children
    in
    if List.length children > 0 then
        td [ style "vertical-align" "middle" ]
            [ input
                [ style "transform" "scale(1.5)"
                , type_ "checkbox"
                , attribute "checked" ""
                , onClick (RemoveOptionWithChildren ( option.parent ++ "/" ++ option.name, children ))
                ]
                []
            ]

    else
        td [ style "vertical-align" "middle" ]
            [ input
                [ style "transform" "scale(1.5)"
                , type_ "checkbox"
                , attribute "checked" ""
                , onClick (RemoveOption (option.parent ++ "/" ++ option.name))
                ]
                []
            ]


uncheckedBox : Model -> OptionNode -> Html Msg
uncheckedBox model option =
    let
        closest_parent =
            getClosestParent option.parent

        chkd =
            if List.member closest_parent (List.map getClosestParent model.select_list) then
                attribute "checked" ""

            else
                class ""

        children =
            unwrapInstallerOption option.children
    in
    if List.length children > 0 then
        td [ style "vertical-align" "middle" ]
            [ input
                [ style "transform" "scale(1.5)"
                , type_ "checkbox"
                , chkd
                , onClick
                    (AddCheckOptionWithChildren
                        ( option.parent
                        , option.name
                        , children
                        )
                    )
                ]
                []
            ]

    else
        td [ style "vertical-align" "middle" ]
            [ input
                [ style "transform" "scale(1.5)"
                , type_ "checkbox"
                , chkd
                , onClick (AddCheckOption ( option.parent, option.name ))
                ]
                []
            ]


locked : OptionNode -> Html Msg
locked _ =
    td [ style "vertical-align" "middle" ]
        [ input [ style "transform" "scale(1.5)", type_ "checkbox", attribute "checked" "", disabled True ] [] ]


parentLocked : Model -> OptionNode -> Html Msg
parentLocked model option =
    let
        closest_parent =
            getClosestParent option.parent

        chkd =
            if List.member closest_parent (List.map getClosestParent model.select_list) then
                attribute "checked" ""

            else
                class ""
    in
    td [ style "vertical-align" "middle" ]
        [ input [ style "transform" "scale(1.5)", type_ "checkbox", chkd, disabled True ] [] ]


unwrapInstallerOption : InstallerOption -> List OptionNode
unwrapInstallerOption option =
    case option of
        InstallerOption l ->
            l
