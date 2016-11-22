// validate account password on change
window.safeLauncher.directive('validateAccountSecret', [ 'CONSTANTS', 'MESSAGES', '$rootScope', '$translate',
  function($constant, $msg, $rootScope, $translate) {
    var onChange = function(scope, ele, attr, ctrl) {
      var msgEle = ele.siblings('.msg');
      var parent = ele.parent();
      var strengthEle = ele.siblings('.strength');
      var statusEle = ele.siblings('.status');
      var value = '';
      var zxcvbn = require('zxcvbn');
      var isValid = false;

      var resetField = function() {
        isValid = false;
        parent.removeClass('error');
        ctrl.$setValidity('fieldValidator', true);
        strengthEle.width('0');
        statusEle.removeClass('icn');
        return msgEle.text('');
      };
      ele.bind('keyup', function(e) {
        ctrl.$setValidity('fieldValidator', false);
        value = e.target.value;
        resetField();
        if (!value) {
          return scope.isPasswordValid({
            result: isValid
          });
        }
        /*jscs:disable requireCamelCaseOrUpperCaseIdentifiers*/
        var log10 = zxcvbn(value).guesses_log10;
        /*jscs:enable requireCamelCaseOrUpperCaseIdentifiers*/
        statusEle.removeClass('icn');
        switch (true) {
          case (log10 < 4):
            // parent.addClass('vweak');
            $translate($msg.PASS_VERY_WEEK).then(function(msg) {
                msgEle.text(msg);
            });
            break;
          case (log10 < 8):
            // parent.addClass('weak');
            $translate($msg.PASS_WEEK).then(function(msg) {
                msgEle.text(msg);
            });
            break;
          case (log10 < 10):
            // parent.addClass('somewhat-secure');
            if (attr.fieldType === 'SECRET') {
              statusEle.addClass('icn');
              isValid = true;
            }
            $translate($msg.PASS_SOMEWHAT_SECURE).then(function(msg) {
                msgEle.text(msg);
            });
            break;
          case (log10 >= 10):
            // parent.addClass('secure');
            statusEle.addClass('icn');
            $translate($msg.PASS_SECURE).then(function(msg) {
                msgEle.text(msg);
            });
            isValid = true;
            break;
          default:
        }
        strengthEle.width(Math.min((log10 / 16) * 100, 100) + '%');
        ctrl.$setValidity('fieldValidator', isValid);
        scope.isPasswordValid({
          result: isValid
        });
        scope.$applyAsync();
      });
    };
    return {
      scope: {
        isPasswordValid: '&'
      },
      restrict: 'A',
      require: '^?ngModel',
      link: onChange
    };
  }
]);
